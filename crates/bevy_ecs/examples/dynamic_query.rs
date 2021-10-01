use bevy_ecs::component::{ComponentDescriptor, StorageType};
use bevy_ecs::prelude::World;
use bevy_ecs::query::dynamic::{DynamicItem, DynamicQuery};

#[derive(PartialEq, Debug)]
struct TestComponent {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(PartialEq, Debug)]
struct GridSpace {
    x: u8,
    y: u8,
}

fn main() {
    let mut world = World::new();
    let test_vector_id = world
        .register_component(ComponentDescriptor::new::<TestComponent>(
            StorageType::Table,
        ))
        .unwrap();

    let test_grid_id = world
        .register_component(ComponentDescriptor::new::<GridSpace>(
            StorageType::SparseSet,
        ))
        .unwrap();

    let query = DynamicQuery::new()
        .entity()
        .mut_component(test_vector_id)
        .without_component(test_grid_id)
        .build();

    for i in 0..10 {
        let mut entity = world.spawn();
        let test = TestComponent {
            x: f64::from(i),
            y: f64::from(i),
            z: f64::from(i),
        };
        entity.insert(test);
        if i % 2 == 0 {
            entity.insert(GridSpace {
                x: i as u8,
                y: i as u8,
            });
        }
    }

    let mut query_state = world.query_dynamic(&query);
    for mut items in query_state.iter_mut(&mut world) {
        unsafe {
            match items.as_mut_slice() {
                [DynamicItem::Entity(entity), DynamicItem::MutableComponent(reference)] => {
                    let vector = reference.downcast_unchecked::<TestComponent>();
                    println!("Entity:{} {:?}", entity.id(), vector);
                    vector.y = vector.x * vector.x;
                    vector.z = vector.x * vector.x;
                    vector.x = vector.x * vector.x;
                }
                _ => unreachable!(),
            }
        }
    }

    let second_query = DynamicQuery::new()
        .component(test_vector_id)
        .optional_component(test_grid_id)
        .build();

    let mut second_query_state = world.query_dynamic(&second_query);
    for items in second_query_state.iter_mut(&mut world) {
        unsafe {
            match items.as_slice() {
                [DynamicItem::Component(vector_reference), DynamicItem::Component(grid_reference)] =>
                {
                    println!(
                        "{:?}, {:?}",
                        vector_reference.downcast_unchecked::<TestComponent>(),
                        grid_reference.downcast_unchecked::<GridSpace>()
                    );
                }
                [DynamicItem::Component(reference), DynamicItem::ComponentNotPresent] => {
                    println!(
                        "{:?}, NoMatch",
                        reference.downcast_unchecked::<TestComponent>()
                    );
                }
                _ => unreachable!(),
            }
        }
    }
}
