//! A Space is an environment that manages game objects,
//! and the only way you can create objects at all in MoleEngine.
//! It implements a variant of the Entity-Component-System pattern.
//!
//! Component storage and Systems are bundled into Features,
//! which are implemented as a user-defined freeform struct
//! that implements `FeatureSet` and parameterizes the Space.
//! This way behaviours a Space supports and dependencies between them
//! are defined at compile time and can be borrow-checked.
//!
//! TODOC: pools, containers, code example

use anymap::AnyMap;
use hibitset::{self as hb, BitSetLike};

use super::{container as cont, Recipe};

/// Trait describing Features of a Space.
/// These determine which component types and behaviors are available in the Space.
/// See the module-level documentation for a full usage example.
///
/// TODOC: containers, init, tick & render
pub trait FeatureSet: 'static + Sized {
    fn init(container_init: cont::Init) -> Self;
    fn tick(&mut self, dt: f32, space: SpaceAccessMut);
    fn draw(&self, space: SpaceAccess);
}

//

/// A handle to an object that can be used to add new components to it.
/// Only given out during object creation.
#[derive(Clone, Copy)]
pub struct MasterKey {
    pub(crate) id: usize,
}

pub struct SpaceAccess<'a> {
    enabled_ids: &'a hb::BitSet,
}

impl<'a> SpaceAccess<'a> {
    pub fn iter(&self) -> cont::IterBuilder<(), &hb::BitSet, impl FnMut(usize) -> ()> {
        cont::IterBuilder {
            bits: self.enabled_ids,
            get: |_| (),
        }
    }
}

pub struct SpaceAccessMut<'a> {
    reserved_ids: &'a mut hb::BitSet,
    enabled_ids: &'a mut hb::BitSet,
}

impl<'a> SpaceAccessMut<'a> {
    pub fn iter(&self) -> cont::IterBuilder<(), &hb::BitSet, impl FnMut(usize) -> ()> {
        cont::IterBuilder {
            bits: self.enabled_ids,
            get: |_| (),
        }
    }
    pub fn spawn<R>(&mut self) {
        unimplemented! {};
    }
    pub fn create_object() {
        unimplemented! {};
    }
    pub fn kill_object() {
        unimplemented! {};
    }
}

/// An environment where game objects live.
///
/// The Space handles reserving and giving out IDs for objects,
/// while all Components are stored and handled inside of Features.
/// See the module-level documentation for a full usage example.
pub struct Space<F: FeatureSet> {
    reserved_ids: hb::BitSet,
    enabled_ids: hb::BitSet,
    next_obj_id: usize,
    capacity: usize,
    pools: AnyMap,
    pub features: F,
}

impl<F: FeatureSet> Space<F> {
    /// Create a Space with a a given maximum capacity.
    ///
    /// Currently this capacity is a hard limit; Spaces do not grow.
    /// The FeatureSet's `init` and `create_pools` functions are called here.
    pub fn with_capacity(capacity: usize) -> Self {
        let mut space = Space {
            reserved_ids: hb::BitSet::with_capacity(capacity as u32),
            enabled_ids: hb::BitSet::with_capacity(capacity as u32),
            next_obj_id: 0,
            capacity,
            pools: AnyMap::new(),
            features: F::init(cont::Init { capacity }),
        };
        // find first index after what pools reserved and start accepting new objects from there
        //
        // negation of the bitset can't not have a first item so we unwrap here.
        // creating a pool will cause a panic if there's not enough room
        // this is a little ugly implementation-wise but a panic is probably always the desirable behavior here
        space.next_obj_id = (!&space.reserved_ids).iter().nth(0).unwrap() as usize;
        space
    }

    /// Create an 'ad-hoc' object in this Space, that is, one that isn't based on a Recipe.
    /// Returns `Some(())` if successful, `None` if there's no room left in the Space.
    pub fn create_object(&mut self, f: impl FnOnce(MasterKey, &mut F)) -> Option<()> {
        let key = self.do_create_object()?;
        f(key, &mut self.features);
        Some(())
    }

    fn do_create_object(&mut self) -> Option<MasterKey> {
        if self.next_obj_id < self.capacity {
            let id = self.next_obj_id;
            self.next_obj_id += 1;
            self.create_object_at(id);
            Some(MasterKey { id })
        } else {
            // find a dead object
            match (!&self.reserved_ids).iter().nth(0) {
                Some(id) if id < self.capacity as u32 => {
                    self.create_object_at(id as usize);
                    Some(MasterKey { id: id as usize })
                }
                _ => None,
            }
        }
    }

    fn create_object_at(&mut self, id: usize) {
        self.reserved_ids.add(id as u32);
        self.enabled_ids.add(id as u32);
    }

    /// Create a pool for a specific Recipe in this Space.
    /// Returns `None` if there's not enough continuous room left in the Space, `Some(())` otherwise.
    ///
    /// This creates all components defined in the Recipe's `spawn_consts` method immediately,
    /// which won't need to be created again when an object is spawned.
    /// If a pool exists it will automatically be used when spawning an object.
    /// Spawning will fail if there's no room left in the pool; in other words,
    /// a pool defines the maximum number of simultaneous instances of the Recipe in the Space.
    pub fn create_pool<R: super::Recipe<F>>(&mut self, size: usize) -> Option<()> {
        let start = self.next_obj_id;
        let end = start + size + 1;
        if end > self.capacity {
            return None;
        }

        let slots: hb::BitSet = (start as u32..end as u32).collect();
        self.next_obj_id = end;
        for id in &slots {
            self.reserved_ids.add(id);
        }

        let pool: Pool<F, R> = Pool::new(slots, &mut self.features);
        self.pools.insert(pool);

        Some(())
    }

    /// Instantiate a Recipe in this Space.
    ///
    /// If a pool exists for that Recipe, uses the pool, otherwise reserves a new object.
    /// Returns `Some(())` if successful, `None` if there's no room in the Pool or Space.
    pub fn spawn<R: super::Recipe<F>>(&mut self, recipe: R) -> Option<()> {
        if let Some(pool) = self.pools.get_mut::<Pool<F, R>>() {
            pool.spawn(recipe, &mut self.enabled_ids, &mut self.features)
        } else {
            self.create_object(|a, feat| {
                R::spawn_consts(a, feat);
                recipe.spawn_vars(a, feat);
            })
        }
    }

    /// Spawn objects described in a RON file into this Space.
    ///
    /// Can fail if either parsing the data fails or all objecs don't fit in the Space.
    pub fn read_ron_file<R>(&mut self, file: std::fs::File) -> Result<(), ron::de::Error>
    where
        R: super::recipe::DeserializeRecipes<F>,
    {
        let mut reader = std::io::BufReader::new(file);
        let mut bytes = Vec::new();
        use std::io::Read;
        reader.read_to_end(&mut bytes)?;

        let mut deser = ron::de::Deserializer::from_bytes(bytes.as_slice())?;
        R::deserialize_into_space(&mut deser, self)
    }

    pub fn tick(&mut self, dt: f32) {
        self.access_features(|f, a| f.tick(dt, a));
    }

    pub fn draw(&self) {
        let access = SpaceAccess {
            enabled_ids: &self.enabled_ids,
        };
        self.features.draw(access);
    }

    pub fn access_features(&mut self, f: impl FnOnce(&mut F, SpaceAccessMut)) {
        let access = SpaceAccessMut {
            reserved_ids: &mut self.reserved_ids,
            enabled_ids: &mut self.enabled_ids,
        };
        f(&mut self.features, access);
    }
}

// Pools

struct Pool<F: FeatureSet, R: Recipe<F>> {
    reserved_slots: hb::BitSet,
    _marker: std::marker::PhantomData<(F, R)>,
}

impl<F: FeatureSet, R: Recipe<F>> Pool<F, R> {
    pub(self) fn new(slots: hb::BitSet, features: &mut F) -> Self {
        for slot in &slots {
            R::spawn_consts(MasterKey { id: slot as usize }, features);
        }
        Pool {
            reserved_slots: slots,
            _marker: std::marker::PhantomData,
        }
    }

    pub(self) fn spawn(
        &mut self,
        recipe: R,
        enabled_ids: &mut hb::BitSet,
        features: &mut F,
    ) -> Option<()> {
        let available_ids = hb::BitSetAnd(&self.reserved_slots, !&*enabled_ids);
        let my_id = available_ids.iter().nth(0)?;
        enabled_ids.add(my_id);
        recipe.spawn_vars(MasterKey { id: my_id as usize }, features);
        Some(())
    }
}
