use crate::archetype::{Archetype, ArchetypeComponentId};
use crate::component::{Component, ComponentId, ComponentTicks, StorageType};
use crate::entity::Entity;
use crate::query::{Access, Fetch, FetchState, FilterFetch, FilteredAccess, WorldQuery};
use crate::schedule::DynEq;
use crate::storage::{ComponentSparseSet, Table, Tables};
use crate::world::World;
use std::alloc::Layout;
use std::cell::UnsafeCell;
use std::ptr;
use std::ptr::NonNull;

#[derive(Debug, Clone)]
pub enum DynamicParam {
    Entity,
    Component { id: ComponentId },
    OptionalComponent { id: ComponentId },
}

#[derive(Debug, Clone)]
pub struct DynamicParamSet {
    pub set: Vec<DynamicParam>,
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
    param: DynamicParam,
}

pub struct DynamicSetFetch {
    params_fetch: Vec<DynamicFetch>,
}

impl WorldQuery for DynamicQuery {
    type Fetch = DynamicSetFetch;
    type State = DynamicSetFetchState;
}

impl WorldQuery for DynamicFilterQuery {
    type Fetch = DynamicFilterFetch;
    type State = ();
}

pub struct DynamicSetFetchState {
    params: Vec<DynamicFetchState>,
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
    type Fetch = DynamicFilterFetch;
    type State = ();

    fn state(&self, world: &World) -> Self::State {
        todo!()
    }
}

pub trait IDynamicQuery {
    type Fetch: for<'world, 'state> Fetch<'world, 'state, State = Self::State>;
    type State: FetchState;

    fn state(&self, world: &World) -> Self::State;
}

pub struct DynamicFilterFetch {
    param: DynamicParam,
    table_components: NonNull<u8>,
}

pub enum DynamicItem {
    Component { pointer: NonNull<u8> },
    Entity(Entity),
    OptionalComponent { pointer: Option<NonNull<u8>> },
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

impl<'w, 's> Fetch<'w, 's> for DynamicFetch {
    type Item = DynamicItem;
    type State = DynamicFetchState;

    unsafe fn init(
        world: &World,
        state: &Self::State,
        last_change_tick: u32,
        change_tick: u32,
    ) -> Self {
        match &state.param {
            DynamicParam::Component { id, .. } => {
                let component_info = world
                    .components
                    .get_info(*id)
                    .expect("Expected component to exist");

                Self::Component {
                    component_id: *id,
                    component_layout: component_info.layout(),
                    storage_type: component_info.storage_type(),
                    table_components: NonNull::dangling(),
                    entities: ptr::null::<Entity>(),
                    entity_table_rows: ptr::null::<usize>(),
                    sparse_set: if component_info.storage_type() == StorageType::SparseSet {
                        world.storages().sparse_sets.get(*id).unwrap()
                    } else {
                        ptr::null::<ComponentSparseSet>()
                    },
                    table_ticks: ptr::null::<UnsafeCell<ComponentTicks>>(),
                    last_change_tick,
                    change_tick,
                }
            }
            DynamicParam::OptionalComponent { id } => {
                let component_info = world
                    .components
                    .get_info(*id)
                    .expect("Expected component to exist");

                Self::OptionalComponent {
                    matches: false,
                    component_id: *id,
                    component_layout: component_info.layout(),
                    storage_type: component_info.storage_type(),
                    table_components: NonNull::dangling(),
                    entities: ptr::null::<Entity>(),
                    entity_table_rows: ptr::null::<usize>(),
                    sparse_set: if component_info.storage_type() == StorageType::SparseSet {
                        world.storages().sparse_sets.get(*id).unwrap()
                    } else {
                        ptr::null::<ComponentSparseSet>()
                    },
                    table_ticks: ptr::null::<UnsafeCell<ComponentTicks>>(),
                    last_change_tick,
                    change_tick,
                }
            }
            DynamicParam::Entity => Self::Entity {
                entities: std::ptr::null(),
            },
            _ => todo!(),
        }
    }

    #[inline]
    fn is_dense(&self) -> bool {
        match self {
            Self::Component {
                storage_type: StorageType::Table,
                ..
            } => true,
            Self::OptionalComponent {
                storage_type: StorageType::Table,
                ..
            } => true,
            Self::Component {
                storage_type: StorageType::SparseSet,
                ..
            } => false,
            Self::OptionalComponent {
                storage_type: StorageType::SparseSet,
                ..
            } => false,
            Self::Entity { .. } => true,
            _ => todo!(),
        }
    }

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        _state: &Self::State,
        archetype: &Archetype,
        tables: &Tables,
    ) {
        match self {
            Self::Component {
                component_id: id,
                storage_type: StorageType::Table,
                ref mut entity_table_rows,
                ref mut table_components,
                ref mut table_ticks,
                ..
            } => {
                *entity_table_rows = archetype.entity_table_rows().as_ptr();
                let column = tables[archetype.table_id()].get_column(*id).unwrap();
                *table_components = column.get_data_ptr();
                *table_ticks = column.get_ticks_ptr();
            }
            Self::Component {
                storage_type: StorageType::SparseSet,
                ref mut entities,
                ..
            }
            | Self::Entity { ref mut entities } => *entities = archetype.entities().as_ptr(),
            _ => todo!(),
        }
    }

    #[inline]
    unsafe fn set_table(&mut self, _state: &Self::State, table: &Table) {
        match self {
            Self::Component {
                component_id: id,
                ref mut table_components,
                ref mut table_ticks,
                ..
            } => {
                let column = table.get_column(*id).unwrap();
                *table_components = column.get_data_ptr().cast::<u8>();
                *table_ticks = column.get_ticks_ptr();
            }
            Self::Entity { ref mut entities } => *entities = table.entities().as_ptr(),
            _ => unimplemented!(),
        }
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Self::Item {
        match self {
            Self::Component {
                component_layout,
                storage_type: StorageType::Table,
                entity_table_rows,
                table_components,
                ..
            } => {
                let table_row = *entity_table_rows.add(archetype_index);
                DynamicItem::Component {
                    pointer: NonNull::new_unchecked(
                        table_components
                            .as_ptr()
                            .add(table_row * component_layout.size()),
                    ),
                }
            }
            Self::Component {
                storage_type: StorageType::SparseSet,
                entities,
                sparse_set,
                ..
            } => {
                let entity = *entities.add(archetype_index);
                let (component, _) = (**sparse_set).get_with_ticks(entity).unwrap();
                DynamicItem::Component {
                    pointer: NonNull::new_unchecked(component),
                }
            }
            Self::Entity { entities } => DynamicItem::Entity(*entities.add(archetype_index)),
            _ => todo!(),
        }
    }

    #[inline]
    unsafe fn table_fetch(&mut self, table_row: usize) -> Self::Item {
        match self {
            Self::Component {
                component_layout,
                table_components,
                ..
            } => {
                return DynamicItem::Component {
                    pointer: NonNull::new_unchecked(
                        table_components
                            .as_ptr()
                            .add(table_row * component_layout.size()),
                    ),
                };
            }
            Self::Entity { entities } => {
                let test = DynamicItem::Entity(*(*entities).add(table_row));
                test
            }
            _ => unimplemented!(),
        }
    }
}

impl<'w, 's> Fetch<'w, 's> for DynamicFilterFetch {
    type Item = bool;
    type State = ();

    unsafe fn init(
        world: &World,
        state: &Self::State,
        last_change_tick: u32,
        change_tick: u32,
    ) -> Self {
        todo!()
    }

    fn is_dense(&self) -> bool {
        todo!()
    }

    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &Archetype,
        tables: &Tables,
    ) {
        todo!()
    }

    unsafe fn set_table(&mut self, state: &Self::State, table: &Table) {
        todo!()
    }

    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Self::Item {
        todo!()
    }

    unsafe fn table_fetch(&mut self, table_row: usize) -> Self::Item {
        todo!()
    }
}

pub struct DynamicFetchState {
    param: DynamicParam,
}

unsafe impl FetchState for DynamicFetchState {
    fn init(_world: &mut World) -> Self {
        unimplemented!()
    }

    fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
        match &self.param {
            DynamicParam::Component { id, .. } => {
                if access.access().has_read(*id) {
                    panic!("Dynamic access conflicts with a previous access in this query. Mutable component access must be unique.");
                }
                access.add_write(*id);
            }
            DynamicParam::Entity => {}
            _ => todo!(),
        }
    }

    fn update_archetype_component_access(
        &self,
        archetype: &Archetype,
        access: &mut Access<ArchetypeComponentId>,
    ) {
        match &self.param {
            DynamicParam::Component { id, .. } => {
                if let Some(archetype_component_id) = archetype.get_archetype_component_id(*id) {
                    access.add_write(archetype_component_id);
                }
            }
            DynamicParam::Entity => {}
            _ => todo!(),
        }
    }

    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        match &self.param {
            DynamicParam::Component { id, .. } => archetype.contains(*id),
            DynamicParam::Entity => true,
            _ => todo!(),
        }
    }

    fn matches_table(&self, table: &Table) -> bool {
        match &self.param {
            DynamicParam::Component { id, .. } => table.has_column(*id),
            DynamicParam::Entity => true,
            _ => todo!(),
        }
    }
}

pub struct DynamicSetFetchItem {
    pub items: Vec<DynamicItem>,
}

impl<'w, 's> Fetch<'w, 's> for DynamicSetFetch {
    type Item = DynamicSetFetchItem;
    type State = DynamicSetFetchState;

    unsafe fn init(
        world: &World,
        state: &Self::State,
        last_change_tick: u32,
        change_tick: u32,
    ) -> Self {
        Self {
            params_fetch: state
                .params
                .iter()
                .map(|s| DynamicFetch::init(world, s, last_change_tick, change_tick))
                .collect(),
        }
    }

    fn is_dense(&self) -> bool {
        self.params_fetch.iter().all(|p| p.is_dense())
    }

    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &Archetype,
        tables: &Tables,
    ) {
        self.params_fetch
            .iter_mut()
            .zip(state.params.iter())
            .for_each(|(p, s)| p.set_archetype(s, archetype, tables))
    }

    unsafe fn set_table(&mut self, state: &Self::State, table: &Table) {
        self.params_fetch
            .iter_mut()
            .zip(state.params.iter())
            .for_each(|(p, s)| p.set_table(s, table))
    }

    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Self::Item {
        DynamicSetFetchItem {
            items: self
                .params_fetch
                .iter_mut()
                .map(|p| p.archetype_fetch(archetype_index))
                .collect(),
        }
    }

    unsafe fn table_fetch(&mut self, table_row: usize) -> Self::Item {
        DynamicSetFetchItem {
            items: self
                .params_fetch
                .iter_mut()
                .map(|p| p.table_fetch(table_row))
                .collect(),
        }
    }
}

unsafe impl FetchState for DynamicSetFetchState {
    fn init(world: &mut World) -> Self {
        unimplemented!()
    }

    fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
        self.params
            .iter()
            .for_each(|p| p.update_component_access(access))
    }

    fn update_archetype_component_access(
        &self,
        archetype: &Archetype,
        access: &mut Access<ArchetypeComponentId>,
    ) {
        self.params
            .iter()
            .for_each(|p| p.update_archetype_component_access(archetype, access))
    }

    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        self.params.iter().all(|p| p.matches_archetype(archetype))
    }

    fn matches_table(&self, table: &Table) -> bool {
        self.params.iter().all(|p| p.matches_table(table))
    }
}
