use crate::archetype::{Archetype, ArchetypeComponentId};
use crate::component::{ComponentId, ComponentTicks, StorageType};
use crate::entity::Entity;
use crate::query::{Access, Fetch, FetchState, FilterFetch, FilteredAccess, WorldQuery};
use crate::storage::{ComponentSparseSet, Table, Tables};
use crate::world::World;
use std::alloc::Layout;
use std::cell::UnsafeCell;
use std::ptr;
use std::ptr::NonNull;

#[derive(Clone)]
pub enum DynamicParam {
    Entity,
    Component { id: ComponentId },
    // ParameterSet {
    //     count: u8,
    //     parameters: Box<[DynamicQuery; 16]>
    // }
}

pub struct DynamicQuery {
    pub param: DynamicParam,
}
pub struct DynamicFilterQuery {
    param: DynamicParam,
}

impl WorldQuery for DynamicQuery {
    type Fetch = DynamicFetch;
    type State = DynamicFetchState;
}

impl WorldQuery for DynamicFilterQuery {
    type Fetch = DynamicFilterFetch;
    type State = ();
}

impl IDynamicQuery for DynamicQuery {
    type Fetch = DynamicFetch;
    type State = DynamicFetchState;

    fn state(&self, world: &World) -> Self::State {
        DynamicFetchState {
            param: self.param.clone(),
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

pub struct DynamicComponent {
    pub component_pointer: NonNull<u8>,
}

pub enum DynamicFetch {
    Entity,
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
}

impl<'w, 's> Fetch<'w, 's> for DynamicFetch {
    type Item = DynamicComponent;
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
            Self::Component {
                storage_type: StorageType::SparseSet,
                ..
            } => false,
            _ => todo!(),
        }
    }

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
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
                component_id: id,
                storage_type: StorageType::SparseSet,
                mut entities,
                ..
            } => entities = archetype.entities().as_ptr(),
            _ => todo!(),
        }
    }

    #[inline]
    unsafe fn set_table(&mut self, state: &Self::State, table: &Table) {
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
                DynamicComponent {
                    component_pointer: NonNull::new_unchecked(
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
                DynamicComponent {
                    component_pointer: NonNull::new_unchecked(component),
                }
            }
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
                return DynamicComponent {
                    component_pointer: NonNull::new_unchecked(
                        table_components
                            .as_ptr()
                            .add(table_row * component_layout.size()),
                    ),
                }
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
            _ => todo!(),
        }
    }

    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        match &self.param {
            DynamicParam::Component { id, .. } => archetype.contains(*id),
            _ => todo!(),
        }
    }

    fn matches_table(&self, table: &Table) -> bool {
        match &self.param {
            DynamicParam::Component { id, .. } => table.has_column(*id),
            _ => todo!(),
        }
    }
}
