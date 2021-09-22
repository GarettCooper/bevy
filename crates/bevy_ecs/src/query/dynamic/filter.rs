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
        if self.without {
            access.add_without(self.component_id)
        } else {
            access.add_with(self.component_id)
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
        archetype.contains(self.component_id) ^ self.without
    }

    #[inline]
    fn matches_table(&self, table: &Table) -> bool {
        table.has_column(self.component_id) ^ self.without
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
        _last_change_tick: u32,
        _change_tick: u32,
    ) -> Self {
        Self {
            storage_type: world
                .components
                .get_info(state.component_id)
                .expect("Expected component to exist")
                .storage_type(),
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
