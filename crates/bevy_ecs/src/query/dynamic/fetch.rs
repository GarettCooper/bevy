use std::cell::UnsafeCell;

use bevy_reflect::erased_serde::private::serde::__private::ptr;
use bevy_reflect::erased_serde::private::serde::__private::ptr::NonNull;

use crate::archetype::{Archetype, ArchetypeComponentId};
use crate::component::{ComponentId, ComponentTicks, StorageType};
use crate::entity::Entity;
use crate::prelude::World;
use crate::query::dynamic::{
    DynamicFetch, DynamicFetchState, DynamicItem, DynamicParam, DynamicSetFetch,
    DynamicSetFetchItem, DynamicSetFetchState,
};
use crate::query::{Access, Fetch, FetchState, FilteredAccess};
use crate::storage::{ComponentSparseSet, Table, Tables};

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
            Self::OptionalComponent {
                ref mut matches,
                component_id: id,
                storage_type: StorageType::Table,
                ref mut entity_table_rows,
                ref mut table_components,
                ref mut table_ticks,
                ..
            } => {
                *matches = DynamicFetchState {
                    param: DynamicParam::Component { id: *id },
                }
                .matches_archetype(archetype);
                if *matches {
                    *entity_table_rows = archetype.entity_table_rows().as_ptr();
                    let column = tables[archetype.table_id()].get_column(*id).unwrap();
                    *table_components = column.get_data_ptr();
                    *table_ticks = column.get_ticks_ptr();
                }
            }
            Self::OptionalComponent {
                ref mut matches,
                component_id,
                storage_type: StorageType::SparseSet,
                ref mut entities,
                ..
            } => {
                *matches = DynamicFetchState {
                    param: DynamicParam::Component { id: *component_id },
                }
                .matches_archetype(archetype);
                if *matches {
                    *entities = archetype.entities().as_ptr()
                }
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
            Self::OptionalComponent {
                ref mut matches,
                component_id: id,
                ref mut table_components,
                ref mut table_ticks,
                ..
            } => {
                *matches = DynamicFetchState {
                    param: DynamicParam::Component { id: *id },
                }
                .matches_table(table);
                if *matches {
                    let column = table.get_column(*id).unwrap();
                    *table_components = column.get_data_ptr().cast::<u8>();
                    *table_ticks = column.get_ticks_ptr();
                }
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
            }
            | Self::OptionalComponent {
                matches: true,
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
            }
            | Self::OptionalComponent {
                matches: true,
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
            Self::OptionalComponent { matches: false, .. } => DynamicItem::NoMatch,
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
            }
            | Self::OptionalComponent {
                matches: true,
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
            Self::OptionalComponent { matches: false, .. } => DynamicItem::NoMatch,
            Self::Entity { entities } => {
                let test = DynamicItem::Entity(*(*entities).add(table_row));
                test
            }
            _ => unimplemented!(),
        }
    }
}

unsafe impl FetchState for DynamicFetchState {
    fn init(_world: &mut World) -> Self {
        unimplemented!()
    }

    #[inline]
    fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
        match &self.param {
            DynamicParam::Component { id, .. } | DynamicParam::OptionalComponent { id, .. } => {
                if access.access().has_read(*id) {
                    panic!("Dynamic access conflicts with a previous access in this query. Mutable component access must be unique.");
                }
                access.add_write(*id);
            }
            DynamicParam::Entity => {}
            _ => todo!(),
        }
    }

    #[inline]
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
            DynamicParam::OptionalComponent { id, .. } => {
                if archetype.contains(*id) {
                    if let Some(archetype_component_id) = archetype.get_archetype_component_id(*id)
                    {
                        access.add_write(archetype_component_id);
                    }
                }
            }
            DynamicParam::Entity => {}
            _ => todo!(),
        }
    }

    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        match &self.param {
            DynamicParam::Component { id, .. } => archetype.contains(*id),
            DynamicParam::OptionalComponent { .. } | DynamicParam::Entity => true,
            _ => todo!(),
        }
    }

    fn matches_table(&self, table: &Table) -> bool {
        match &self.param {
            DynamicParam::Component { id, .. } => table.has_column(*id),
            DynamicParam::OptionalComponent { .. } | DynamicParam::Entity => true,
            _ => todo!(),
        }
    }
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

    #[inline]
    fn is_dense(&self) -> bool {
        self.params_fetch.iter().all(|p| p.is_dense())
    }

    #[inline]
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

    #[inline]
    unsafe fn set_table(&mut self, state: &Self::State, table: &Table) {
        self.params_fetch
            .iter_mut()
            .zip(state.params.iter())
            .for_each(|(p, s)| p.set_table(s, table))
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Self::Item {
        DynamicSetFetchItem {
            items: self
                .params_fetch
                .iter_mut()
                .map(|p| p.archetype_fetch(archetype_index))
                .collect(),
        }
    }

    #[inline]
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

    #[inline]
    fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
        self.params
            .iter()
            .for_each(|p| p.update_component_access(access))
    }

    #[inline]
    fn update_archetype_component_access(
        &self,
        archetype: &Archetype,
        access: &mut Access<ArchetypeComponentId>,
    ) {
        self.params
            .iter()
            .for_each(|p| p.update_archetype_component_access(archetype, access))
    }

    #[inline]
    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        self.params.iter().all(|p| p.matches_archetype(archetype))
    }

    #[inline]
    fn matches_table(&self, table: &Table) -> bool {
        self.params.iter().all(|p| p.matches_table(table))
    }
}
