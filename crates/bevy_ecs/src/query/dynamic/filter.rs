use crate::archetype::{Archetype, ArchetypeComponentId};
use crate::component::{ComponentId, StorageType};
use crate::prelude::World;
use crate::query::dynamic::{
    DynamicFilterFetch, DynamicFilterState, DynamicSetFilterFetch, DynamicSetFilterState,
};
use crate::query::{Access, Fetch, FetchState, FilteredAccess};
use crate::storage::{Table, Tables};

unsafe impl FetchState for DynamicFilterState {
    fn init(_world: &mut World) -> Self {
        unimplemented!()
    }

    #[inline]
    fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
        match self {
            DynamicFilterState::WithOrWithout {
                without,
                component_id,
            } => {
                if *without {
                    access.add_without(*component_id)
                } else {
                    access.add_with(*component_id)
                }
            }
            DynamicFilterState::Or(set) => set.update_component_access(access),
        }
    }

    #[inline]
    fn update_archetype_component_access(
        &self,
        _archetype: &Archetype,
        _access: &mut Access<ArchetypeComponentId>,
    ) {
    }

    #[inline]
    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        match self {
            DynamicFilterState::WithOrWithout {
                component_id,
                without,
            } => archetype.contains(*component_id) ^ *without,
            DynamicFilterState::Or(set) => {
                set.params.iter().any(|f| f.matches_archetype(archetype))
            }
        }
    }

    #[inline]
    fn matches_table(&self, table: &Table) -> bool {
        match self {
            DynamicFilterState::WithOrWithout {
                component_id,
                without,
            } => table.has_column(*component_id) ^ *without,
            DynamicFilterState::Or(set) => set.params.iter().any(|f| f.matches_table(table)),
        }
    }
}

unsafe impl FetchState for DynamicSetFilterState {
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

impl<'w, 's> Fetch<'w, 's> for DynamicSetFilterFetch {
    type Item = bool;
    type State = DynamicSetFilterState;

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
                .map(|f| DynamicFilterFetch::init(world, f, last_change_tick, change_tick))
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
        self.params_fetch
            .iter_mut()
            .all(|p| p.archetype_fetch(archetype_index))
    }

    #[inline]
    unsafe fn table_fetch(&mut self, table_row: usize) -> Self::Item {
        self.params_fetch
            .iter_mut()
            .all(|p| p.table_fetch(table_row))
    }
}

impl<'w, 's> Fetch<'w, 's> for DynamicFilterFetch {
    type Item = bool;
    type State = DynamicFilterState;

    unsafe fn init(
        world: &World,
        state: &Self::State,
        last_change_tick: u32,
        change_tick: u32,
    ) -> Self {
        Self {
            storage_type: match state {
                DynamicFilterState::WithOrWithout { component_id, .. } => world
                    .components
                    .get_info(*component_id)
                    .expect("Expected component to exist")
                    .storage_type(),
                DynamicFilterState::Or(set) => {
                    if set
                        .params
                        .iter()
                        .map(|s| Self::init(world, s, last_change_tick, change_tick))
                        .all(|f| f.storage_type == StorageType::Table)
                    {
                        StorageType::Table
                    } else {
                        StorageType::SparseSet
                    }
                }
            },
        }
    }

    #[inline]
    fn is_dense(&self) -> bool {
        self.storage_type == StorageType::Table
    }

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        _state: &Self::State,
        _archetype: &Archetype,
        _tables: &Tables,
    ) {
    }

    #[inline]
    unsafe fn set_table(&mut self, _state: &Self::State, _table: &Table) {}

    #[inline]
    unsafe fn archetype_fetch(&mut self, _archetype_index: usize) -> Self::Item {
        true
    }

    #[inline]
    unsafe fn table_fetch(&mut self, _table_row: usize) -> Self::Item {
        true
    }
}
