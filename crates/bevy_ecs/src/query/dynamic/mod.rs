use std::alloc::Layout;
use std::cell::UnsafeCell;

use std::ptr::NonNull;

use crate::component::{ComponentId, ComponentTicks, StorageType};
use crate::entity::Entity;
use crate::query::{Fetch, FetchState, WorldQuery};

use crate::storage::ComponentSparseSet;
use crate::world::World;

mod fetch;
mod filter;

#[derive(Debug, Clone)]
enum DynamicParam {
    Entity,
    Component { component_id: ComponentId },
    OptionalComponent { component_id: ComponentId },
}

#[derive(Debug, Clone)]
enum DynamicFilter {
    With { component_id: ComponentId },
    Without { component_id: ComponentId },
}

#[derive(Debug, Clone)]
struct DynamicParamSet {
    set: Box<[DynamicParam]>,
}

#[derive(Debug, Clone)]
struct DynamicFilterSet {
    set: Box<[DynamicFilter]>,
}

pub struct DynamicQuery {
    params: DynamicParamSet,
}

impl DynamicQuery {
    pub fn new() -> DynamicQueryBuilder {
        DynamicQueryBuilder { params: Vec::new() }
    }
}

pub struct DynamicQueryBuilder {
    params: Vec<DynamicParam>,
}

impl DynamicQueryBuilder {
    pub fn entity(&mut self) -> &mut Self {
        self.params.push(DynamicParam::Entity);
        self
    }

    pub fn component(&mut self, component_id: ComponentId) -> &mut Self {
        self.params.push(DynamicParam::Component { component_id });
        self
    }

    pub fn optional_component(&mut self, component_id: ComponentId) -> &mut Self {
        self.params
            .push(DynamicParam::OptionalComponent { component_id });
        self
    }

    pub fn build(&mut self) -> DynamicQuery {
        DynamicQuery {
            params: DynamicParamSet {
                set: self.params.clone().into_boxed_slice(),
            },
        }
    }
}

pub struct DynamicFilterQuery {
    params: DynamicFilterSet,
}

impl DynamicFilterQuery {
    pub fn new() -> DynamicFilterQueryBuilder {
        DynamicFilterQueryBuilder {
            conditions: Vec::new(),
        }
    }
}

pub struct DynamicFilterQueryBuilder {
    conditions: Vec<DynamicFilter>,
}

impl DynamicFilterQueryBuilder {
    pub fn with_component(&mut self, component_id: ComponentId) -> &mut Self {
        self.conditions.push(DynamicFilter::With { component_id });
        self
    }

    pub fn without_component(&mut self, component_id: ComponentId) -> &mut Self {
        self.conditions
            .push(DynamicFilter::Without { component_id });
        self
    }

    pub fn build(&mut self) -> DynamicFilterQuery {
        DynamicFilterQuery {
            params: DynamicFilterSet {
                set: self.conditions.clone().into_boxed_slice(),
            },
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

pub struct DynamicFilterState {
    component_id: ComponentId,
    without: bool,
}

pub struct DynamicSetFilterState {
    params: Box<[DynamicFilterState]>,
}

impl IDynamicQuery for DynamicQuery {
    type Fetch = DynamicSetFetch;
    type State = DynamicSetFetchState;

    fn state(&self, _world: &World) -> Self::State {
        DynamicSetFetchState {
            params: self
                .params
                .set
                .iter()
                .map(|p| DynamicFetchState { param: p.clone() })
                .collect(),
        }
    }
}

impl DynamicParam {
    fn get_layout(&self, world: &World) -> Layout {
        match self {
            DynamicParam::Entity => Layout::new::<Entity>(),
            DynamicParam::Component { component_id: id } => {
                world.components.get_info(*id).unwrap().layout()
            }
            DynamicParam::OptionalComponent { .. } => unimplemented!(),
        }
    }
}

impl DynamicParamSet {
    pub fn get_layout(&self, world: &World) -> (Layout, Vec<usize>) {
        let mut iter = self.set.iter();
        let mut offsets = vec![0];
        let mut full_layout = iter.next().unwrap().get_layout(world);
        for param in iter {
            let (layout, offset) = full_layout.extend(param.get_layout(world)).unwrap();
            full_layout = layout;
            offsets.push(offset)
        }
        (full_layout, offsets)
    }
}

impl IDynamicQuery for DynamicFilterQuery {
    type Fetch = DynamicSetFilterFetch;
    type State = DynamicSetFilterState;

    fn state(&self, _world: &World) -> Self::State {
        DynamicSetFilterState {
            params: self
                .params
                .set
                .iter()
                .map(|f| match f {
                    DynamicFilter::With { component_id } => DynamicFilterState {
                        component_id: *component_id,
                        without: false,
                    },
                    DynamicFilter::Without { component_id } => DynamicFilterState {
                        component_id: *component_id,
                        without: true,
                    },
                })
                .collect(),
        }
    }
}

pub trait IDynamicQuery {
    type Fetch: for<'world, 'state> Fetch<'world, 'state, State = Self::State>;
    type State: FetchState;

    fn state(&self, world: &World) -> Self::State;
}

pub enum DynamicItem {
    Component { pointer: NonNull<u8> },
    Entity(Entity),
    NoMatch,
}

pub enum DynamicFetch {
    Entity {
        entities: *const Entity,
    },
    Component {
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
    OptionalComponent {
        matches: bool,
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

pub struct DynamicSetFetchItem {
    pub items: Box<[DynamicItem]>,
}
