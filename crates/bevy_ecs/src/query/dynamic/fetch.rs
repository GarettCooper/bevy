use crate::archetype::{Archetype, ArchetypeComponentId};
use crate::component::{ComponentId, ComponentTicks, StorageType};
use crate::entity::Entity;
use crate::query::dynamic::{
    DynamicComponentReference, DynamicFetch, DynamicFetchState, DynamicItem,
    DynamicMutComponentReference, DynamicParam, DynamicQueryEntity, DynamicSetFetch,
    DynamicSetFetchState,
};
use crate::query::{Access, Fetch, FetchState, FilteredAccess};
use crate::storage::{ComponentSparseSet, Table, Tables};
use crate::world::World;
use core::ptr;
use std::cell::UnsafeCell;
use std::ptr::NonNull;

impl<'w, 's> Fetch<'w, 's> for DynamicFetch {
    type Item = DynamicItem<'w>;
    type State = DynamicFetchState;

    unsafe fn init(
        world: &World,
        state: &Self::State,
        last_change_tick: u32,
        change_tick: u32,
    ) -> Self {
        match &state.param {
            DynamicParam::Component {
                component_id,
                mutable,
                optional,
            } => {
                let component_info = world
                    .components
                    .get_info(*component_id)
                    .expect("Expected component to exist");

                Self::Component {
                    mutable: *mutable,
                    optional: *optional,
                    matches: false,
                    type_id: component_info
                        .type_id()
                        .expect("Expected component to have Type ID"),
                    component_id: *component_id,
                    component_layout: component_info.layout(),
                    storage_type: component_info.storage_type(),
                    table_components: NonNull::dangling(),
                    entities: ptr::null::<Entity>(),
                    entity_table_rows: ptr::null::<usize>(),
                    sparse_set: if component_info.storage_type() == StorageType::SparseSet {
                        world.storages().sparse_sets.get(*component_id).unwrap()
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
            Self::Entity { .. } => true,
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
                ref mut matches,
                optional,
                ..
            } => {
                *matches = !*optional || archetype.contains(*id);
                if *matches {
                    *entity_table_rows = archetype.entity_table_rows().as_ptr();
                    let column = tables[archetype.table_id()].get_column(*id).unwrap();
                    *table_components = column.get_data_ptr();
                    *table_ticks = column.get_ticks_ptr();
                }
            }
            Self::Component {
                component_id: id,
                storage_type: StorageType::SparseSet,
                ref mut entities,
                ref mut matches,
                optional,
                ..
            } => {
                *matches = !*optional || archetype.contains(*id);
                if *matches {
                    *entities = archetype.entities().as_ptr()
                }
            }
            Self::Entity { ref mut entities } => *entities = archetype.entities().as_ptr(),
        }
    }

    #[inline]
    unsafe fn set_table(&mut self, _state: &Self::State, table: &Table) {
        match self {
            Self::Component {
                component_id: id,
                ref mut table_components,
                ref mut table_ticks,
                ref mut matches,
                optional,
                ..
            } => {
                *matches = !*optional || table.has_column(*id);
                if *matches {
                    let column = table.get_column(*id).unwrap();
                    *table_components = column.get_data_ptr().cast::<u8>();
                    *table_ticks = column.get_ticks_ptr();
                }
            }
            Self::Entity { ref mut entities } => *entities = table.entities().as_ptr(),
        }
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Self::Item {
        match self {
            Self::Component {
                optional: true,
                matches: false,
                ..
            } => DynamicItem::ComponentNotPresent,
            Self::Component {
                component_layout,
                storage_type: StorageType::Table,
                entity_table_rows,
                table_components,
                type_id,
                mutable,
                ..
            } => {
                let table_row = *entity_table_rows.add(archetype_index);
                let pointer = NonNull::new_unchecked(
                    table_components
                        .as_ptr()
                        .add(table_row * component_layout.size())
                        .cast::<()>(),
                );

                if *mutable {
                    DynamicItem::MutableComponent(DynamicMutComponentReference {
                        type_id: *type_id,
                        pointer,
                        phantom: Default::default(),
                    })
                } else {
                    DynamicItem::Component(DynamicComponentReference {
                        type_id: *type_id,
                        pointer,
                        phantom: Default::default(),
                    })
                }
            }
            Self::Component {
                storage_type: StorageType::SparseSet,
                entities,
                sparse_set,
                mutable,
                type_id,
                ..
            } => {
                let entity = *entities.add(archetype_index);
                let (component, _) = (**sparse_set).get_with_ticks(entity).unwrap();
                let pointer = NonNull::new_unchecked(component.cast::<()>());

                if *mutable {
                    DynamicItem::MutableComponent(DynamicMutComponentReference {
                        type_id: *type_id,
                        pointer,
                        phantom: Default::default(),
                    })
                } else {
                    DynamicItem::Component(DynamicComponentReference {
                        type_id: *type_id,
                        pointer,
                        phantom: Default::default(),
                    })
                }
            }
            Self::Entity { entities } => DynamicItem::Entity(*entities.add(archetype_index)),
        }
    }

    #[inline]
    unsafe fn table_fetch(&mut self, table_row: usize) -> Self::Item {
        match self {
            Self::Component {
                optional: true,
                matches: false,
                ..
            } => DynamicItem::ComponentNotPresent,
            Self::Component {
                component_layout,
                table_components,
                type_id,
                mutable,
                ..
            } => {
                let pointer = NonNull::new_unchecked(
                    table_components
                        .as_ptr()
                        .add(table_row * component_layout.size())
                        .cast::<()>(),
                );
                if *mutable {
                    DynamicItem::MutableComponent(DynamicMutComponentReference {
                        type_id: *type_id,
                        pointer,
                        phantom: Default::default(),
                    })
                } else {
                    DynamicItem::Component(DynamicComponentReference {
                        type_id: *type_id,
                        pointer,
                        phantom: Default::default(),
                    })
                }
            }
            Self::Entity { entities } => DynamicItem::Entity(*(*entities).add(table_row)),
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
            DynamicParam::Component {
                component_id: id,
                mutable: true,
                ..
            } => {
                if access.access().has_read(*id) {
                    panic!("Dynamic access conflicts with a previous access in this query. Mutable component access must be unique.");
                }

                access.add_write(*id);
            }
            DynamicParam::Component {
                component_id: id,
                mutable: false,
                ..
            } => {
                if access.access().has_write(*id) {
                    panic!("Dynamic access conflicts with a previous access in this query. Mutable component access must be unique.");
                }
                access.add_read(*id);
            }
            DynamicParam::Entity => {}
        }
    }

    #[inline]
    fn update_archetype_component_access(
        &self,
        archetype: &Archetype,
        access: &mut Access<ArchetypeComponentId>,
    ) {
        match &self.param {
            DynamicParam::Component {
                component_id: id,
                optional,
                mutable,
                ..
            } => {
                if !*optional || archetype.contains(*id) {
                    if let Some(archetype_component_id) = archetype.get_archetype_component_id(*id)
                    {
                        if *mutable {
                            access.add_write(archetype_component_id);
                        } else {
                            access.add_read(archetype_component_id);
                        }
                    }
                }
            }
            DynamicParam::Entity => {}
        }
    }

    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        match &self.param {
            DynamicParam::Component {
                component_id: id,
                optional: false,
                ..
            } => archetype.contains(*id),
            DynamicParam::Component { optional: true, .. } | DynamicParam::Entity => true,
        }
    }

    fn matches_table(&self, table: &Table) -> bool {
        match &self.param {
            DynamicParam::Component {
                component_id: id,
                optional: false,
                ..
            } => table.has_column(*id),
            DynamicParam::Component { optional: true, .. } | DynamicParam::Entity => true,
        }
    }
}

impl<'w, 's> Fetch<'w, 's> for DynamicSetFetch {
    type Item = DynamicQueryEntity<'w>;
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
        DynamicQueryEntity {
            items: self
                .params_fetch
                .iter_mut()
                .map(|p| p.archetype_fetch(archetype_index))
                .collect(),
        }
    }

    #[inline]
    unsafe fn table_fetch(&mut self, table_row: usize) -> Self::Item {
        DynamicQueryEntity {
            items: self
                .params_fetch
                .iter_mut()
                .map(|p| p.table_fetch(table_row))
                .collect(),
        }
    }
}

unsafe impl FetchState for DynamicSetFetchState {
    fn init(_world: &mut World) -> Self {
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
