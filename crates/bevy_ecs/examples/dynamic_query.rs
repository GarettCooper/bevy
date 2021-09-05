use bevy_ecs::prelude::World;
use bevy_ecs::query::dynamic::{DynamicQuery, DynamicParam};
use bevy_ecs::component::{ComponentDescriptor, StorageType};

#[derive(PartialEq, Debug)]
struct TestComponent {
    x: f64,
    y: f64,
    z: f64
}

fn main() {
    let mut world = World::new();
    let id = world.register_component(ComponentDescriptor::new::<TestComponent>(StorageType::Table)).unwrap();

    let query = DynamicQuery {
        param: DynamicParam::Component {
            id
        }
    };

    for i in 0..10 {
        let mut entity = world.spawn();
        let test = TestComponent { x: f64::from(i), y: f64::from(i), z: f64::from(i) };
        entity.insert(test);
    }

    let mut query_state = world.query_dynamic(&query);
    for component in query_state.iter_mut(&mut world) {
        unsafe {
            println!("{:?}", *component.component_pointer.cast::<TestComponent>().as_ptr());
            let reference = &mut *component.component_pointer.cast::<TestComponent>().as_ptr();
            reference.y = reference.x * reference.x;
            reference.z = reference.x * reference.x;
            reference.x = reference.x * reference.x;
        }
    }

    let mut second_query_state = world.query_dynamic(&query);
    for component in second_query_state.iter_mut(&mut world) {
        unsafe {
            println!("{:?}", *component.component_pointer.cast::<TestComponent>().as_ptr());
        }
    }
}