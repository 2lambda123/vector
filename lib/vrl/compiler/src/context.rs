use core::Resolved;
use std::{cell::RefCell, rc::Rc};
use vector_common::TimeZone;

use crate::{state::Runtime, Target};

pub struct Context<'a> {
    target: &'a mut dyn Target,
    state: &'a mut Runtime,
    timezone: &'a TimeZone,
}

impl<'a> Context<'a> {
    /// Create a new [`Context`].
    pub fn new(target: &'a mut dyn Target, state: &'a mut Runtime, timezone: &'a TimeZone) -> Self {
        Self {
            target,
            state,
            timezone,
        }
    }

    /// Get a reference to the [`Target`].
    #[must_use]
    pub fn target(&self) -> &dyn Target {
        self.target
    }

    /// Get a mutable reference to the [`Target`].
    pub fn target_mut(&mut self) -> &mut dyn Target {
        self.target
    }

    /// Get a reference to the [`runtime state`](Runtime).
    #[must_use]
    pub fn state(&self) -> &Runtime {
        self.state
    }

    /// Get a mutable reference to the [`runtime state`](Runtime).
    pub fn state_mut(&mut self) -> &mut Runtime {
        self.state
    }

    /// Get a reference to the [`TimeZone`]
    #[must_use]
    pub fn timezone(&self) -> &TimeZone {
        self.timezone
    }
}

#[derive(Clone)]
pub struct BatchContext<'a> {
    indices: Vec<usize>,
    resolved_values: Vec<Resolved>,
    targets: Vec<Rc<RefCell<dyn Target + 'a>>>,
    states: Vec<Rc<RefCell<Runtime>>>,
    timezone: TimeZone,
}

impl<'a> BatchContext<'a> {
    /// Create a new [`BatchContext`].
    pub fn new(
        indices: Vec<usize>,
        resolved_values: Vec<Resolved>,
        targets: Vec<Rc<RefCell<dyn Target + 'a>>>,
        states: Vec<Rc<RefCell<Runtime>>>,
        timezone: TimeZone,
    ) -> Self {
        Self {
            indices,
            resolved_values,
            targets,
            states,
            timezone,
        }
    }

    pub fn empty_with_timezone(timezone: TimeZone) -> Self {
        Self {
            indices: Vec::new(),
            resolved_values: Vec::new(),
            targets: Vec::new(),
            states: Vec::new(),
            timezone,
        }
    }

    pub fn len(&self) -> usize {
        self.resolved_values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.resolved_values.is_empty()
    }

    pub fn indices(&mut self) -> &[usize] {
        &self.indices
    }

    pub fn resolved_values_mut(&mut self) -> &mut Vec<Resolved> {
        &mut self.resolved_values
    }

    pub fn targets(&self) -> impl Iterator<Item = Rc<RefCell<dyn Target + 'a>>> + '_ {
        self.targets.iter().cloned()
    }

    pub fn states(&self) -> impl Iterator<Item = Rc<RefCell<Runtime>>> + '_ {
        self.states.iter().cloned()
    }

    pub fn drain_filter<F>(&mut self, mut filter: F) -> Self
    where
        F: FnMut(&mut Resolved) -> bool,
    {
        let mut this_indices = Vec::new();
        let mut this_resolved_values = Vec::new();
        let mut this_targets = Vec::new();
        let mut this_states = Vec::new();
        let mut other_indices = Vec::new();
        let mut other_resolved_values = Vec::new();
        let mut other_targets = Vec::new();
        let mut other_states = Vec::new();

        std::mem::swap(&mut self.indices, &mut this_indices);
        std::mem::swap(&mut self.resolved_values, &mut this_resolved_values);
        std::mem::swap(&mut self.targets, &mut this_targets);
        std::mem::swap(&mut self.states, &mut this_states);

        for (((index, mut resolved), target), state) in this_indices
            .into_iter()
            .zip(this_resolved_values)
            .zip(this_targets)
            .zip(this_states)
        {
            if filter(&mut resolved) {
                other_indices.push(index);
                other_resolved_values.push(resolved);
                other_targets.push(target);
                other_states.push(state);
            } else {
                self.indices.push(index);
                self.resolved_values.push(resolved);
                self.targets.push(target);
                self.states.push(state);
            }
        }

        Self {
            indices: other_indices,
            resolved_values: other_resolved_values,
            targets: other_targets,
            states: other_states,
            timezone: self.timezone,
        }
    }

    pub fn filtered<P>(self, mut predicate: P) -> Self
    where
        P: FnMut(&Resolved) -> bool,
    {
        let (((indices, resolved_values), targets), states) = self
            .indices
            .into_iter()
            .zip(self.resolved_values)
            .zip(self.targets)
            .zip(self.states)
            .filter(|(((_, value), _), _)| predicate(value))
            .unzip();

        Self {
            indices,
            resolved_values,
            targets,
            states,
            timezone: self.timezone,
        }
    }

    pub fn extend(&mut self, other: BatchContext<'a>) {
        assert_eq!(self.timezone, other.timezone);
        self.indices.extend(other.indices);
        self.resolved_values.extend(other.resolved_values);
        self.targets.extend(other.targets);
        self.states.extend(other.states);
    }

    pub fn timezone(&self) -> TimeZone {
        self.timezone
    }

    pub fn iter_mut<'b>(
        &'b mut self,
    ) -> impl Iterator<
        Item = (
            usize,
            &'b mut Resolved,
            Rc<RefCell<dyn Target + 'a>>,
            Rc<RefCell<Runtime>>,
            TimeZone,
        ),
    > {
        let indices = self.indices.iter();
        let resolved_values = self.resolved_values.iter_mut();
        let targets = self.targets.iter();
        let states = self.states.iter();
        let timezone = self.timezone;

        BatchContextIterMut {
            indices,
            resolved_values,
            targets,
            states,
            timezone,
        }
    }

    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        Vec<usize>,
        Vec<Resolved>,
        Vec<Rc<RefCell<dyn Target + 'a>>>,
        Vec<Rc<RefCell<Runtime>>>,
        TimeZone,
    ) {
        (
            self.indices,
            self.resolved_values,
            self.targets,
            self.states,
            self.timezone,
        )
    }
}

pub struct BatchContextIterMut<'a, 'b> {
    indices: std::slice::Iter<'b, usize>,
    resolved_values: std::slice::IterMut<'b, Resolved>,
    targets: std::slice::Iter<'b, Rc<RefCell<dyn Target + 'a>>>,
    states: std::slice::Iter<'b, Rc<RefCell<Runtime>>>,
    timezone: TimeZone,
}

impl<'a, 'b> Iterator for BatchContextIterMut<'a, 'b> {
    type Item = (
        usize,
        &'b mut Resolved,
        Rc<RefCell<dyn Target + 'a>>,
        Rc<RefCell<Runtime>>,
        TimeZone,
    );

    fn next(&mut self) -> Option<Self::Item> {
        Some((
            *self.indices.next()?,
            self.resolved_values.next()?,
            self.targets.next()?.clone(),
            self.states.next()?.clone(),
            self.timezone,
        ))
    }
}
