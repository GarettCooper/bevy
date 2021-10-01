use std::alloc::Layout;
use std::cell::UnsafeCell;

use std::ptr::NonNull;

use crate::component::{ComponentId, ComponentTicks, StorageType};
use crate::entity::Entity;
use crate::query::WorldQuery;

use crate::storage::ComponentSparseSet;
use std::any::TypeId;
use std::marker::PhantomData;
use std::ops::Index;
use std::slice::{Iter, IterMut};

mod fetch;
mod filter;

pub struct DynamicQuery {
    params: DynamicParamSet,
    filter: DynamicFilterSet,
}

impl DynamicQuery {
    pub fn new() -> DynamicQueryBuilder {
        DynamicQueryBuilder {
            params: Vec::new(),
            conditions: Or::new(),
        }
    }

    pub(crate) fn fetch_state(&self) -> DynamicSetFetchState {
        DynamicSetFetchState {
            params: self
                .params
                .set
                .iter()
                .map(|p| DynamicFetchState { param: p.clone() })
                .collect(),
        }
    }

    pub(crate) fn filter_state(&self) -> DynamicSetFilterState {
        DynamicSetFilterState {
            params: self.filter.set.iter().map(|p| p.clone().into()).collect(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum DynamicParam {
    Entity,
    Component {
        component_id: ComponentId,
        optional: bool,
        mutable: bool,
    },
}

#[derive(Debug, Clone)]
enum DynamicFilter {
    With { component_id: ComponentId },
    Without { component_id: ComponentId },
    Or(DynamicFilterSet),
}

#[derive(Debug, Clone)]
struct DynamicParamSet {
    set: Box<[DynamicParam]>,
}

#[derive(Debug, Clone)]
struct DynamicFilterSet {
    set: Box<[DynamicFilter]>,
}

pub struct DynamicQueryBuilder {
    params: Vec<DynamicParam>,
    conditions: Or,
}

impl DynamicQueryBuilder {
    pub fn entity(&mut self) -> &mut Self {
        self.params.push(DynamicParam::Entity);
        self
    }

    pub fn component(&mut self, component_id: ComponentId) -> &mut Self {
        self.params.push(DynamicParam::Component {
            component_id,
            optional: false,
            mutable: false,
        });
        self
    }

    pub fn mut_component(&mut self, component_id: ComponentId) -> &mut Self {
        self.params.push(DynamicParam::Component {
            component_id,
            optional: false,
            mutable: true,
        });
        self
    }

    pub fn optional_component(&mut self, component_id: ComponentId) -> &mut Self {
        self.params.push(DynamicParam::Component {
            component_id,
            optional: true,
            mutable: false,
        });
        self
    }

    pub fn optional_mut_component(&mut self, component_id: ComponentId) -> &mut Self {
        self.params.push(DynamicParam::Component {
            component_id,
            optional: true,
            mutable: true,
        });
        self
    }

    pub fn with_component(&mut self, component_id: ComponentId) -> &mut Self {
        self.conditions.with_component(component_id);
        self
    }

    pub fn without_component(&mut self, component_id: ComponentId) -> &mut Self {
        self.conditions.without_component(component_id);
        self
    }

    pub fn build(&self) -> DynamicQuery {
        DynamicQuery {
            params: DynamicParamSet {
                set: self.params.clone().into_boxed_slice(),
            },
            filter: self.conditions.build(),
        }
    }
}

/// Marker struct for QueryState
pub struct DynamicFilterQuery {}

impl DynamicFilterQuery {
    pub fn new() -> Or {
        Or {
            conditions: Vec::new(),
        }
    }
}

pub struct Or {
    conditions: Vec<DynamicFilter>,
}

impl Or {
    pub fn new() -> Self {
        Self {
            conditions: Vec::new(),
        }
    }

    pub fn with_component(&mut self, component_id: ComponentId) -> &mut Self {
        self.conditions.push(DynamicFilter::With { component_id });
        self
    }

    pub fn without_component(&mut self, component_id: ComponentId) -> &mut Self {
        self.conditions
            .push(DynamicFilter::Without { component_id });
        self
    }

    pub fn or(&mut self, conditions: &Or) {
        self.conditions.push(DynamicFilter::Or(conditions.build()))
    }

    fn build(&self) -> DynamicFilterSet {
        DynamicFilterSet {
            set: self.conditions.clone().into_boxed_slice(),
        }
    }
}

pub struct DynamicSetFetch {
    params_fetch: Box<[DynamicFetch]>,
}

pub struct DynamicSetFilterFetch {
    params_fetch: Box<[DynamicFilterFetch]>,
}

impl WorldQuery for DynamicQuery {
    type Fetch = DynamicSetFetch;
    type State = DynamicSetFetchState;
}

impl WorldQuery for DynamicFilterQuery {
    type Fetch = DynamicSetFilterFetch;
    type State = DynamicSetFilterState;
}

pub struct DynamicSetFetchState {
    params: Box<[DynamicFetchState]>,
}

pub struct DynamicFilterFetch {
    storage_type: StorageType,
}

pub enum DynamicFilterState {
    WithOrWithout {
        component_id: ComponentId,
        without: bool,
    },
    Or(DynamicSetFilterState),
}

impl From<DynamicFilter> for DynamicFilterState {
    fn from(filter: DynamicFilter) -> Self {
        match filter {
            DynamicFilter::With { component_id } => Self::WithOrWithout {
                component_id,
                without: false,
            },
            DynamicFilter::Without { component_id } => Self::WithOrWithout {
                component_id,
                without: true,
            },
            DynamicFilter::Or(dynamic_filter_set) => Self::Or(DynamicSetFilterState {
                params: dynamic_filter_set
                    .set
                    .iter()
                    .map(|f| f.clone().into())
                    .collect(),
            }),
        }
    }
}

pub struct DynamicSetFilterState {
    params: Box<[DynamicFilterState]>,
}

pub enum DynamicItem<'a> {
    Entity(Entity),
    Component(DynamicComponentReference<'a>),
    MutableComponent(DynamicMutComponentReference<'a>),
    ComponentNotPresent,
}

pub struct DynamicComponentReference<'a> {
    type_id: TypeId,
    pointer: NonNull<()>,
    phantom: PhantomData<&'a ()>,
}

impl<'a> DynamicComponentReference<'a> {
    pub fn downcast<T: 'static>(&self) -> Option<&'a T> {
        if TypeId::of::<T>() != self.type_id {
            None
        } else {
            // SAFE Type Ids match. Technically unsound, but Type ID collision isn't likely enough to worry about.
            // We also have guaranteed mutable access
            unsafe { Some(&*self.pointer.as_ptr().cast::<T>()) }
        }
    }

    #[inline(always)]
    pub unsafe fn downcast_unchecked<T>(&self) -> &'a T {
        &*(self.pointer.as_ptr().cast::<T>())
    }

    #[inline(always)]
    pub fn component_ptr(&self) -> NonNull<()> {
        self.pointer
    }

    #[inline(always)]
    pub fn component_type_id(&self) -> TypeId {
        self.type_id
    }
}

pub struct DynamicMutComponentReference<'a> {
    type_id: TypeId,
    pointer: NonNull<()>,
    phantom: PhantomData<&'a mut ()>,
}

impl<'a> DynamicMutComponentReference<'a> {
    pub fn downcast<T: 'static>(&mut self) -> Option<&'a mut T> {
        if TypeId::of::<T>() != self.type_id {
            None
        } else {
            // SAFE Type Ids match. Technically unsound, but Type ID collision isn't likely enough to worry about.
            // We also have guaranteed mutable access
            unsafe { Some(&mut *(self.pointer.as_ptr().cast::<T>())) }
        }
    }

    #[inline(always)]
    pub unsafe fn downcast_unchecked<T>(&mut self) -> &'a mut T {
        &mut *(self.pointer.as_ptr().cast::<T>())
    }

    #[inline(always)]
    pub fn component_ptr(&self) -> NonNull<()> {
        self.pointer
    }

    #[inline(always)]
    pub fn component_type_id(&self) -> TypeId {
        self.type_id
    }
}

pub enum DynamicFetch {
    Entity {
        entities: *const Entity,
    },
    Component {
        mutable: bool,
        optional: bool,
        matches: bool,
        type_id: TypeId,
        component_id: ComponentId,
        component_layout: Layout,
        storage_type: StorageType,
        table_components: NonNull<u8>,
        table_ticks: *const UnsafeCell<ComponentTicks>,
        entities: *const Entity,
        entity_table_rows: *const usize,
        sparse_set: *const ComponentSparseSet,
        last_change_tick: u32,
        change_tick: u32,
    },
}

#[derive(Debug)]
pub struct DynamicFetchState {
    param: DynamicParam,
}

pub struct DynamicQueryEntity<'a> {
    pub items: Box<[DynamicItem<'a>]>,
}

impl<'a> Index<usize> for DynamicQueryEntity<'a> {
    type Output = DynamicItem<'a>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.items[index]
    }
}

impl<'a> DynamicQueryEntity<'a> {
    pub fn iter(&self) -> Iter<'a, DynamicItem> {
        self.items.iter()
    }

    pub fn iter_mut(&mut self) -> IterMut<'a, DynamicItem> {
        self.items.iter_mut()
    }

    pub fn as_slice(&self) -> &'a [DynamicItem] {
        &self.items
    }

    pub fn as_mut_slice(&mut self) -> &'a mut [DynamicItem] {
        &mut self.items
    }
}
