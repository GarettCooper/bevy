use std::alloc::Layout;
use std::cell::UnsafeCell;
use std::iter::Filter;
use std::ptr;
use std::ptr::NonNull;

use crate::archetype::{Archetype, ArchetypeComponentId};
use crate::component::{Component, ComponentId, ComponentTicks, StorageType};
use crate::entity::Entity;
use crate::query::{Access, Fetch, FetchState, FilterFetch, FilteredAccess, WorldQuery};
use crate::schedule::DynEq;
use crate::storage::{ComponentSparseSet, Table, Tables};
use crate::world::World;

mod fetch;
mod filter;

#[derive(Debug, Clone)]
pub enum DynamicParam {
    Entity,
    Component { id: ComponentId },
    OptionalComponent { id: ComponentId },
}

#[derive(Debug, Clone)]
pub enum DynamicFilter {
    With { component_id: ComponentId },
    Without { component_id: ComponentId },
}

#[derive(Debug, Clone)]
pub struct DynamicParamSet {
    pub set: Vec<DynamicParam>,
}

#[derive(Debug, Clone)]
pub struct DynamicFilterSet {
    pub set: Vec<DynamicFilter>,
}

pub struct DynamicQuery {
    pub params: DynamicParamSet,
}

impl DynamicQuery {
    pub fn new(params: DynamicParamSet) -> Self {
        Self { params }
    }
}

pub struct DynamicFilterQuery {
    pub params: DynamicFilterSet,
}

pub struct DynamicSetFetch {
    params_fetch: Vec<DynamicFetch>,
}

pub struct DynamicSetFilterFetch {
    params_fetch: Vec<DynamicFilterFetch>,
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
    params: Vec<DynamicFetchState>,
}

pub struct DynamicFilterFetch {
    storage_type: StorageType,
}

pub struct DynamicFilterState {
    component_id: ComponentId,
    without: bool,
}

pub struct DynamicSetFilterState {
    params: Vec<DynamicFilterState>,
}

impl IDynamicQuery for DynamicQuery {
    type Fetch = DynamicSetFetch;
    type State = DynamicSetFetchState;

    fn state(&self, world: &World) -> Self::State {
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
    pub items: Vec<DynamicItem>,
}
