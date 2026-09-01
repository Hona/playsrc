use super::*;

#[derive(Clone, Copy)]
struct Estimate {
    distance: f32,
    request: f32,
    request_input: CollisionRequest,
}
#[derive(Clone, Copy)]
struct SavedPose {
    core: u64,
    orientation: CoreOrientation,
    angular: [f32; 3],
}
struct ImpactEpisode {
    group: u64,
    pairs: Vec<[u64; 2]>,
    known: Vec<u64>,
    pushed: Vec<u64>,
    saved: Vec<SavedPose>,
    estimates: BTreeMap<u64, Option<Estimate>>,
    repetitions: BTreeMap<u64, u16>,
}

impl PhysicsEnvironment {
    fn episode_pair(&self, group: u64, cores: [u64; 2]) -> Option<&CoreContactPair> {
        self.contacts
            .groups
            .get(&group)?
            .pairs
            .iter()
            .find(|pair| pair.cores == cores || pair.cores == [cores[1], cores[0]])
    }
    fn refresh_episode_pair(
        &mut self,
        episode: &mut ImpactEpisode,
        cores: [u64; 2],
        except: Option<u64>,
    ) -> Result<bool, EnvironmentError> {
        let Some(pair) = self.episode_pair(episode.group, cores) else {
            return Ok(false);
        };
        let ids = pair.contacts.clone();
        for id in ids.into_iter().rev() {
            if Some(id) == except {
                continue;
            }
            self.refresh_contact(id)?;
            episode.estimates.remove(&id);
            episode.repetitions.remove(&id);
            if self.contacts.contacts[&id].surface.broken {
                self.remove_contact(id)?;
            }
        }
        Ok(self
            .episode_pair(episode.group, cores)
            .is_some_and(|pair| !pair.contacts.is_empty()))
    }
    fn push_episode_core(
        &mut self,
        episode: &mut ImpactEpisode,
        core: u64,
        except: [u64; 2],
    ) -> Result<(), EnvironmentError> {
        episode.pushed.push(core);
        let pairs = self.contacts.groups[&episode.group]
            .pairs
            .iter()
            .rev()
            .filter(|pair| {
                pair.cores.contains(&core)
                    && pair.cores != except
                    && pair.cores != [except[1], except[0]]
            })
            .map(|pair| pair.cores)
            .collect::<Vec<_>>();
        for pair in pairs {
            if episode.pairs.contains(&pair) {
                continue;
            }
            if self.refresh_episode_pair(episode, pair, None)? {
                episode.pairs.push(pair);
            }
        }
        Ok(())
    }
    fn estimate_contact_impact(&self, id: u64) -> Result<Option<Estimate>, EnvironmentError> {
        let contact = self
            .contacts
            .contacts
            .get(&id)
            .ok_or(ContactError::Missing)?;
        if contact.surface.distance > self.tolerances.maximum_impact_distance {
            return Ok(None);
        }
        let indices = self.contact_bodies(contact.endpoints)?;
        let bodies = self.contact_body_states(indices);
        let request_input = CollisionRequest {
            contact_distance: contact.surface.distance,
            contact_threshold: self.tolerances.collision_distance,
            rotations: std::array::from_fn(|side| {
                bodies[side].map(|body| crate::CollisionRotation {
                    angular_velocity: body.angular_velocity,
                    contact_radius: self.bodies[indices[side]].physical.radius,
                })
            }),
            inverse_timestep: (1.0 / f64::from(self.config.timestep)) as f32,
        };
        let request = request_input.speed()?;
        let mut closing = 0.0;
        for (side, body) in bodies.into_iter().enumerate() {
            let Some(body) = body else {
                continue;
            };
            let jacobian = contact.normal_jacobians[side].ok_or(ContactError::Missing)?;
            let angular = crate::response::ordered_dot(jacobian, body.angular_velocity);
            let linear = crate::response::ordered_dot(contact.surface.normal, body.linear_velocity);
            closing += if side == 0 {
                f64::from(angular) + f64::from(linear)
            } else {
                f64::from(-angular) - f64::from(linear)
            };
        }
        let distance = (f64::from(contact.surface.distance)
            - (closing + f64::from(request * 0.5_f32)) * f64::from(self.config.timestep))
            as f32;
        if !distance.is_finite() {
            return Err(EnvironmentError::NonFinite);
        }
        Ok(Some(Estimate {
            distance,
            request,
            request_input,
        }))
    }
    fn synchronize_episode_core(
        &mut self,
        episode: &mut ImpactEpisode,
        core: u64,
    ) -> Result<(), EnvironmentError> {
        if episode.known.contains(&core) {
            return Ok(());
        }
        episode.known.push(core);
        let index = self.core_body_index(core)?;
        let body = &self.bodies[index];
        if !body.motion_enabled
            || !self
                .islands
                .movement(core)
                .is_some_and(crate::CoreMovement::is_simulated)
        {
            return Ok(());
        }
        let phase = body
            .motion_phase()
            .ok_or(EnvironmentError::MissingMotionPhase)?;
        episode.saved.push(SavedPose {
            core,
            orientation: phase.next_orientation,
            angular: body.velocity.angular,
        });
        let now = self.time();
        self.bodies[index].synchronize_collision_pose(now)?;
        Ok(())
    }
    pub(in crate::world) fn finish_contact_impact(
        &mut self,
        id: u64,
        source_pair: u64,
    ) -> Result<(), EnvironmentError> {
        let initial = self
            .contacts
            .contacts
            .get(&id)
            .ok_or(ContactError::Missing)?
            .clone();
        let initial_pair = self
            .episode_pair(initial.group, initial.cores)
            .ok_or(ContactError::Missing)?
            .cores;
        let mut episode = ImpactEpisode {
            group: initial.group,
            pairs: Vec::new(),
            known: Vec::new(),
            pushed: Vec::new(),
            saved: Vec::new(),
            estimates: BTreeMap::new(),
            repetitions: BTreeMap::new(),
        };
        for side in 0..2 {
            let core = initial.cores[side];
            if self.bodies[self.core_body_index(core)?].is_moveable() {
                self.push_episode_core(&mut episode, core, initial_pair)?;
            }
            let core = initial_pair[side];
            if self.bodies[self.core_body_index(core)?].is_moveable() {
                episode.known.push(core);
            }
        }
        episode.pairs.push(initial_pair);
        self.refresh_episode_pair(&mut episode, initial_pair, Some(id))?;
        let mut propagations = 0;
        loop {
            let mut selected = None;
            let mut minimum = self.tolerances.collision_distance;
            for pair in episode.pairs.clone().into_iter().rev() {
                let Some(pair) = self.episode_pair(episode.group, pair) else {
                    continue;
                };
                let indices = [
                    self.core_body_index(pair.cores[0])?,
                    self.core_body_index(pair.cores[1])?,
                ];
                if indices.iter().all(|index| {
                    self.bodies[*index].kind == BodyKind::Static
                        || self.bodies[*index].temporarily_frozen
                }) {
                    continue;
                }
                let pair_cores = pair.cores;
                let ids = pair.contacts.clone();
                for contact in ids.into_iter().rev() {
                    if let std::collections::btree_map::Entry::Vacant(entry) =
                        episode.estimates.entry(contact)
                    {
                        entry.insert(self.estimate_contact_impact(contact)?);
                    }
                    if let Some(estimate) = episode.estimates[&contact]
                        && estimate.distance < minimum
                    {
                        minimum = estimate.distance;
                        selected = Some((contact, pair_cores, estimate));
                    }
                }
            }
            let Some((contact, pair, estimate)) = selected else {
                break;
            };
            let retained = self.contacts.contacts[&contact].clone();
            for core in retained.cores.into_iter().rev() {
                if self.bodies[self.core_body_index(core)?].kind != BodyKind::Static {
                    self.synchronize_episode_core(&mut episode, core)?;
                }
            }
            let repetitions = episode.repetitions.entry(contact).or_default();
            *repetitions = repetitions
                .checked_add(1)
                .ok_or(EnvironmentError::ClockOverflow)?;
            let dynamic = retained
                .cores
                .map(|core| self.bodies[self.core_body_index(core).unwrap()].is_moveable());
            if !dynamic.iter().any(|value| *value) {
                return Err(EnvironmentError::DisabledMotion);
            }
            let side = usize::from(!dynamic[0]);
            let order = [side, 1 - side];
            let materials =
                self.contact_materials(retained.endpoints, retained.contact.binding.features())?;
            let input = BodyCollision {
                body: retained.endpoints[side].body,
                opposing: retained.endpoints[1 - side].body,
                body_offset: retained.contact.synchronized_offsets[side],
                opposing_offset: retained.contact.synchronized_offsets[1 - side],
                normal: if side == 0 {
                    retained.surface.normal.map(|v| -v)
                } else {
                    retained.surface.normal
                },
                request_speed: Some(estimate.request),
            };
            let result = self.apply_material_collision(
                input,
                Some(order.map(|i| materials[i])),
                *repetitions,
            )?;
            if let Some(observations) = &mut self.collision_observations {
                observations
                    .last_mut()
                    .ok_or(ContactError::Missing)?
                    .request_input = Some(estimate.request_input);
            }
            self.statistics.propagated_impacts = self
                .statistics
                .propagated_impacts
                .checked_add(1)
                .ok_or(EnvironmentError::ClockOverflow)?;
            for source_side in (0..2).rev() {
                let core = retained.cores[source_side];
                let result_side = usize::from(source_side != side);
                if !result.applied[result_side]
                    || !self.bodies[self.core_body_index(core)?].is_moveable()
                {
                    continue;
                }
                if episode.pushed.contains(&core) {
                    episode.estimates.retain(|id, _| {
                        !self
                            .contacts
                            .contacts
                            .get(id)
                            .is_some_and(|contact| contact.cores.contains(&core))
                    });
                } else {
                    self.push_episode_core(&mut episode, core, pair)?;
                }
            }
            propagations += 1;
            if propagations > 5000 {
                self.retire_impact_pair(source_pair)?;
                break;
            }
        }
        for saved in episode.saved.into_iter().rev() {
            if episode.pushed.contains(&saved.core) {
                continue;
            }
            let index = self.core_body_index(saved.core)?;
            let body = &mut self.bodies[index];
            body.orientation = saved.orientation;
            body.velocity.angular = saved.angular;
        }
        let now = self.time();
        let mut ranges = Vec::new();
        for core in episode.pushed.iter().rev() {
            let index = self.core_body_index(*core)?;
            let body = &mut self.bodies[index];
            body.velocity = body
                .motion_profile(self.config)
                .constrain_velocity(body.velocity)?;
            let previous = body
                .motion_phase()
                .ok_or(EnvironmentError::MissingMotionPhase)?;
            let remaining = (previous.end - now) as f32;
            if remaining <= 0.0 {
                body.motion_phase = None;
                continue;
            }
            let (next_orientation, motion) = body.integrate_rotation(remaining, false)?;
            body.retain_integrated_phase(
                BodyMotionPhase {
                    position: body.core_position,
                    prior_orientation: body.orientation,
                    next_orientation,
                    projection_velocity: body.velocity.linear,
                    start: now,
                    end: previous.end,
                    inverse_step: (1.0 / f64::from(remaining)) as f32,
                },
                motion,
            )?;
            if body.movement_range.is_due() {
                ranges.push(*core);
            }
        }
        self.dispatch_ranges(&ranges)?;
        self.refresh_impact_pairs(&episode.pushed)?;
        let identities = {
            let journal = self.collisions.last().ok_or(ContactError::Missing)?;
            [journal.body, journal.opposing]
        };
        let indices = self.contact_bodies(identities.map(|body| BodyConvex { body, convex: 0 }))?;
        let after = indices.map(|index| self.bodies[index].velocity);
        let queued = indices.map(|index| self.bodies[index].queued_velocity);
        let journal = self.collisions.last_mut().unwrap();
        journal.after = after;
        journal.queued_after = queued;
        Ok(())
    }
}
