use bevy_ecs::component::{ComponentDescriptor, StorageType};
use bevy_ecs::prelude::World;
use bevy_ecs::query::dynamic::{
    DynamicFilter, DynamicFilterQuery, DynamicFilterQueryBuilder, DynamicFilterSet, DynamicItem,
    DynamicParam, DynamicParamSet, DynamicQuery, DynamicQueryBuilder,
};

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
        .component(test_vector_id)
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
    for items in query_state.iter_mut(&mut world) {
        unsafe {
            match items.items.as_slice() {
                [DynamicItem::Entity(entity), DynamicItem::Component { pointer }] => {
                    println!(
                        "Entity:{} {:?}",
                        entity.id(),
                        *pointer.cast::<TestComponent>().as_ptr()
                    );
                    let reference = &mut *pointer.cast::<TestComponent>().as_ptr();
                    reference.y = reference.x * reference.x;
                    reference.z = reference.x * reference.x;
                    reference.x = reference.x * reference.x;
                }
                _ => unreachable!(),
            }
        }
    }

    let second_query = DynamicQuery::new().component(test_vector_id).build();

    let filter_query = DynamicFilterQuery::new()
        .without_component(test_grid_id)
        .build();

    println!(
        "Test vector id: {:?}, test grid id: {:?}",
        test_vector_id, test_grid_id
    );

    let mut second_query_state = world.query_dynamic_filtered(&second_query, &filter_query);
    for items in second_query_state.iter_mut(&mut world) {
        unsafe {
            match items.items.as_slice() {
                [DynamicItem::Component {
                    pointer: vector_pointer,
                }, DynamicItem::Component {
                    pointer: grid_pointer,
                }] => {
                    println!(
                        "{:?}, {:?}",
                        *vector_pointer.cast::<TestComponent>().as_ptr(),
                        *grid_pointer.cast::<GridSpace>().as_ptr()
                    );
                }
                [DynamicItem::Component {
                    pointer: vector_pointer,
                }, DynamicItem::NoMatch] => {
                    println!(
                        "{:?}, NoMatch",
                        *vector_pointer.cast::<TestComponent>().as_ptr()
                    );
                }
                [DynamicItem::Component {
                    pointer: vector_pointer,
                }, ..] => {
                    println!("{:?}", *vector_pointer.cast::<TestComponent>().as_ptr());
                }
                _ => unreachable!(),
            }
        }
    }
}
