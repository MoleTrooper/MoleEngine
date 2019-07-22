use super::{
    super::{
        integrator::{Integrator, IntegratorState},
        CollisionEvent, RigidBody,
    },
    broadphase::{BroadPhase, Collidable},
    narrowphase::intersection_check,
    Collider, Transform,
};
use crate::ecs::{event::EventQueue, system::*, IdType, Space};
use std::{collections::HashMap, marker::PhantomData};

/// A System that calculates movement for rigid bodies
/// while taking collisions into account.
/// Integrators and broad phase algorithms are interchangeable.
pub struct CollisionSolver<I, B>
where
    I: Integrator,
    B: BroadPhase,
{
    timestep: f32,
    iterations: usize,
    integrator_marker: PhantomData<I>,
    broad_phase_marker: PhantomData<B>,
}

impl<I, B> CollisionSolver<I, B>
where
    I: Integrator,
    B: BroadPhase,
{
    /// Create a CollisionSolver with the given timestep value.
    /// When used with a constant timestep this can be called once and stored;
    /// otherwise timestep should be updated every frame either by creating
    /// a new solver with this function or using `set_timestep`.
    pub fn with_timestep(timestep: f32, iterations: usize) -> Self {
        CollisionSolver {
            timestep,
            iterations,
            integrator_marker: PhantomData,
            broad_phase_marker: PhantomData,
        }
    }

    /// Set the timestep on an exising CollisionSolver.
    pub fn set_timestep(&mut self, timestep: f32) {
        self.timestep = timestep;
    }
}

impl<'a, I, B> System<'a> for CollisionSolver<I, B>
where
    I: Integrator,
    B: BroadPhase,
{
    type Filter = RigidBodyFilter<'a>;

    fn run_system(&mut self, items: &mut [Self::Filter], space: &Space, queue: &mut EventQueue) {
        // easy way to relate immutable collision pairs back to mutable items
        let id_index_map: HashMap<IdType, usize> = items
            .iter()
            .enumerate()
            .map(|(index, item)| (item.id, index))
            .collect();

        let mut integrator = I::begin_step(self.timestep);

        while let IntegratorState::NeedsDerivatives = integrator.substep(
            items
                .iter_mut()
                .map(|rbf| (&mut *rbf.tr, &mut rbf.body.velocity)),
        ) {
            let iter = items.iter().filter_map(|rbf| {
                rbf.coll.map(|coll| Collidable {
                    id: rbf.id,
                    tr: rbf.tr,
                    coll: coll,
                })
            });

            let mut events = Vec::new();

            let pairs = B::pairs(iter);
            let contacts: Vec<_> = pairs
                .iter()
                .filter_map(|(o1, o2)| intersection_check(*o1, *o2).map(|c| (o1.id, o2.id, c)))
                .collect();

            for _ in 0..self.iterations {
                for (o1_id, o2_id, contact) in &contacts {
                    // every id is in the map so this can't fail
                    let i1 = *id_index_map.get(o1_id).unwrap();
                    let i2 = *id_index_map.get(o2_id).unwrap();
                    // ids guaranteed unequal -> we can do this trick to get mutable ref to both
                    let (o1, o2) = if i1 < i2 {
                        let (l, r) = items.split_at_mut(i2);
                        (&mut l[i1], &mut r[0])
                    } else {
                        let (l, r) = items.split_at_mut(i1);
                        (&mut r[0], &mut l[i2])
                    };

                    contact.manifold.for_each(|p| {
                        let offset_1 = *p - o1.tr.get_translation();
                        let offset_2 = *p - o2.tr.get_translation();

                        let offset_cross_normal_1 =
                            offset_1[0] * contact.normal[1] - contact.normal[0] * offset_1[1];
                        let offset_cross_normal_2 =
                            offset_2[0] * contact.normal[1] - contact.normal[0] * offset_2[1];

                        let normal_vel_1 = o1.body.velocity.linear.dot(&contact.normal)
                            + (offset_cross_normal_1 * o1.body.velocity.angular);
                        // normal is towards obj2 -> this one will be negative
                        // (if objects moving into each other)
                        let normal_vel_2 = o2.body.velocity.linear.dot(&contact.normal)
                            + (offset_cross_normal_2 * o2.body.velocity.angular);

                        let relative_normal_vel = normal_vel_1 - normal_vel_2;
                        if relative_normal_vel < 0.0 {
                            // TODO: clamped per-contact impulse accumulators instead of early out
                            return;
                        }

                        let inv_mass_sum = o1.body.mass.get_inv()
                            + o1.body.moment_of_inertia.get_inv()
                            + o2.body.mass.get_inv()
                            + o2.body.moment_of_inertia.get_inv();

                        let impulse_magnitude = relative_normal_vel / inv_mass_sum; // TODO: restitution -> bounce

                        // apply the impulse

                        o1.body.velocity.linear -=
                            o1.body.mass.get_inv() * impulse_magnitude * *contact.normal;
                        o1.body.velocity.angular -= o1.body.moment_of_inertia.get_inv()
                            * impulse_magnitude
                            * offset_cross_normal_1;
                        o2.body.velocity.linear +=
                            o2.body.mass.get_inv() * impulse_magnitude * *contact.normal;
                        o2.body.velocity.angular += o2.body.moment_of_inertia.get_inv()
                            * impulse_magnitude
                            * offset_cross_normal_2;
                    });
                }
            }

            // events
            // TODO: only generate these if listeners are present?
            for (o1, o2, contact) in &contacts {
                let evt1 = CollisionEvent {
                    source: *o1,
                    other: *o2,
                    normal: -contact.normal,
                    depth: contact.depth,
                    manifold: contact.manifold,
                };
                let evt2 = CollisionEvent {
                    source: *o2,
                    other: *o1,
                    normal: contact.normal,
                    depth: contact.depth,
                    manifold: contact
                        .manifold
                        .map(|p| p - contact.depth * *contact.normal),
                };

                events.push(evt1);
                events.push(evt2);

                queue.push(Box::new(evt1));
                queue.push(Box::new(evt2));
            }

            // for visualization, TODO: remove when all collider types are done and shown to work
            space.write_global_state(|colls| {
                std::mem::replace(colls, events);
            });
        }
    }
}

#[derive(ComponentFilter)]
pub struct RigidBodyFilter<'a> {
    #[id]
    id: IdType,
    tr: &'a mut Transform,
    body: &'a mut RigidBody,
    coll: Option<&'a Collider>,
}
