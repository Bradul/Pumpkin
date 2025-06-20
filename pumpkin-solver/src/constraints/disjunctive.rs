use std::fmt::Debug;
use crate::engine::propagation::{EnqueueDecision, LocalId, PropagationContextMut, Propagator, PropagatorInitialisationContext};
use super::Constraint;
use crate::{predicate, pumpkin_assert_simple, ConstraintOperationError, Solver};
use crate::variables::{IntegerVariable, Literal, TransformableVariable};
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::num::NonZero;
use std::ops::Deref;
use enumset::enum_set;
use crate::basic_types::{Inconsistency, PropagationStatusCP, PropositionalConjunction};
use crate::constraints::theta_lambda_trees::ThetaLambdaTree;
use crate::engine::{DomainEvents, IntDomainEvent};
use crate::engine::opaque_domain_event::OpaqueDomainEvent;
use crate::engine::propagation::contexts::PropagationContextWithTrailedValues;

/// Creates the [Disjunctive](https://sofdem.github.io/gccat/gccat/Cdisjunctive.html) [`Constraint`].
///
/// This constraint ensures that at no point in time the provided task can overlap. This can be
/// seen as a special case of the `cumulative` constraint with capacity 1.
///
/// The length of `start_times` and `durations` should be the same; if
/// this is not the case then this method will panic.
pub fn disjunctive<StartTimes, Durations>(
    start_times: StartTimes,
    durations: Durations,
) -> impl Constraint
where
    StartTimes: IntoIterator,
    StartTimes::Item: IntegerVariable + Debug + 'static,
    StartTimes::IntoIter: ExactSizeIterator,
    Durations: IntoIterator<Item = i32>,
    Durations::IntoIter: ExactSizeIterator,
{
    let start_times = start_times.into_iter().collect::<Vec<_>>();
    let durations = durations.into_iter().collect::<Vec<_>>();

    pumpkin_assert_simple!(start_times.len() == durations.len());
    
    Disjunctive::new(start_times, durations)
}

pub(crate) struct DisjunctiveTask<Var: IntegerVariable + 'static> {
    pub(crate) start_times: Var,
    pub(crate) duration: i32,
    pub(crate) local_id: LocalId,
}

impl<Var: IntegerVariable> DisjunctiveTask<Var> {
    /*pub(super) fn new(start_times: Var, duration: i32, local_id: LocalId) -> Self {
        Self {
            start_times,
            duration,
            local_id
        }
    }

    pub(super) fn get_domain_id(&self) -> DomainId {
        self.start_times.domain_id()
    }*/
}

pub(crate) struct Disjunctive<Var: IntegerVariable + 'static> {
    pub(crate) tasks: Vec<DisjunctiveTask<Var>>,
    pub(crate) reverse_tasks: Vec<DisjunctiveTask<<<Var as IntegerVariable>::AffineView as IntegerVariable>::AffineView>>,
}

impl<Var: IntegerVariable + 'static> Disjunctive<Var> {
    pub(super) fn new(tasks: Vec<Var>, durations: Vec<i32>) -> Self {
        let n = tasks.len();
        let mut disjunctive_tasks = Vec::with_capacity(n);
        let mut reverse_tasks = Vec::with_capacity(n);
        for i in 0..n {
            disjunctive_tasks.push(DisjunctiveTask{
                start_times: tasks[i].clone(),
                duration: durations[i],
                local_id: LocalId::from(i as u32)
            });
            reverse_tasks.push(DisjunctiveTask{
                start_times: tasks[i].offset(durations[i]).scaled(-1),
                duration: durations[i],
                local_id: LocalId::from(i as u32)
            });
        }
        Self {
            tasks: disjunctive_tasks,
            reverse_tasks,
        }
    }
}

impl<Var: IntegerVariable + 'static> Constraint for Disjunctive<Var> {
    fn post(self, solver: &mut Solver, tag: Option<NonZero<u32>>) -> Result<(), ConstraintOperationError> {
        DisjunctivePropagator::new(self.tasks, self.reverse_tasks).post(solver, tag)
    }

    fn implied_by(self, solver: &mut Solver, reification_literal: Literal, tag: Option<NonZero<u32>>) -> Result<(), ConstraintOperationError> {
        DisjunctivePropagator::new(self.tasks, self.reverse_tasks).implied_by(solver, reification_literal, tag)
    }
}

pub(crate) struct DisjunctivePropagator<Var: IntegerVariable + 'static> {
    pub(crate) tasks: Vec<DisjunctiveTask<Var>>,
    pub(crate) reverse_tasks: Vec<DisjunctiveTask<<<Var as IntegerVariable>::AffineView as IntegerVariable>::AffineView>>,
}

impl<Var: IntegerVariable + 'static> DisjunctivePropagator<Var> {
    pub(crate) fn new(tasks: Vec<DisjunctiveTask<Var>>, reverse_tasks: Vec<DisjunctiveTask<<<Var as IntegerVariable>::AffineView as IntegerVariable>::AffineView>>) -> Self {
        Self {
            tasks,
            reverse_tasks,
        }
    }

    pub(crate) fn from_vectors(start_times: Vec<Var>, durations: Vec<i32>) -> Self {
        let disj = Disjunctive::new(start_times, durations);
        Self {
            tasks: disj.tasks,
            reverse_tasks: disj.reverse_tasks
        }
    }
}

impl<Var: IntegerVariable + 'static> Propagator for DisjunctivePropagator<Var> {
    fn name(&self) -> &str {
        "Disjunctive-Edge-Finding"
    }

    fn debug_propagate_from_scratch(&self, mut context: PropagationContextMut) -> PropagationStatusCP {
        let first_result = edge_finding(&self.tasks, &mut context);
        if first_result.is_err() {
            return first_result;
        }
        edge_finding(&self.reverse_tasks, &mut context)
    }

    fn propagate(&mut self, mut context: PropagationContextMut) -> PropagationStatusCP {
        let first_result = edge_finding(&self.tasks, &mut context);
        if first_result.is_err() {
            return first_result;
        }
        edge_finding(&self.reverse_tasks, &mut context)
    }

    fn notify(&mut self, _context: PropagationContextWithTrailedValues, _local_id: LocalId, _event: OpaqueDomainEvent) -> EnqueueDecision {
        EnqueueDecision::Enqueue
    }

    fn initialise_at_root(&mut self, context: &mut PropagatorInitialisationContext) -> Result<(), PropositionalConjunction> {
        self.tasks.iter().for_each(|task| {
            let _ = context.register(
                task.start_times.clone(),
                DomainEvents::BOUNDS,
                task.local_id,
            );
        });
        Ok(())
    }
}

fn compute_overload_explanation<Var: IntegerVariable + 'static>(
    tree: ThetaLambdaTree<Var>,
    lct: i32,
    context: &mut PropagationContextMut
) -> PropagationStatusCP {
    let mut tasks = tree.get_all_theta();
    let mut est = tasks[0].0.lower_bound(context.assignments);
    let mut sum_p = tree.sum_p();
    let mut delta = sum_p - (lct - est) - 1;
    for i in 0..(tasks.len()-1) {
        if delta >= 0 {
            break;
        }
        let (_,d,b) = tasks[i];
        if !b {
            continue;
        } else {
            tasks[i].2 = false;
            sum_p -= d;
            est = tasks[i+1].0.lower_bound(context.assignments);
            delta = sum_p - (lct - est) - 1;
        }
    }
    let mut offset_left = delta / 2;
    if est >= 0 && est - offset_left < 0 {
        offset_left = est;
    }
    let mut offset_right = delta - offset_left;
    if est < 0 && lct + offset_right > 0 {
        offset_left += lct + offset_right;
        offset_right = lct;
    }
    let reason = tasks.iter().flat_map(|(t,d,b)| {
        if *b {
            vec!(
                predicate![t >= est - offset_left],
                predicate![t <= lct + offset_right - d]
            )
        } else {
            vec!()
        }
    }).collect::<PropositionalConjunction>();
    Err(Inconsistency::Conflict(reason))
}

fn compute_edge_finding_explanations<Var: IntegerVariable + 'static>(
    tree: &ThetaLambdaTree<Var>,
    curr_task: &Var,
    curr_task_p: i32,
    new_lb: i32,
    context: &mut PropagationContextMut,
    lct: i32,
) -> PropositionalConjunction {
    let mut tasks = tree.get_all_theta();
    let curr_task_lb = curr_task.lower_bound(context.assignments);
    let mut i = 0;
    let mut sum_p = tree.sum_p();
    let mut delta = std::cmp::min(curr_task_lb, tasks[i].0.lower_bound(context.assignments)) + sum_p + curr_task_p;
    while i < tasks.len() && delta <= lct {
        sum_p -= tasks[i].1;
        i += 1;
        if i == tasks.len() {
            break;
        }
        delta = std::cmp::min(curr_task_lb, tasks[i].0.lower_bound(context.assignments)) + sum_p + curr_task_p;
    }
    if i == tasks.len() {
        panic!("Omega should not be empty.")
    }
    
    let mut j = i;
    let mut sum_p_prime = sum_p;
    while j < tasks.len() {
        if tree.ect() == tasks[j].0.lower_bound(context.assignments) + sum_p_prime {
            break;
        }
        sum_p_prime -= tasks[j].1;
        j += 1;
    }
    if j == tasks.len() {
        panic! ("Omega' should not be empty.");
    }
    let mut reason = PropositionalConjunction::new(vec![]);
    let r = std::cmp::min(curr_task_lb, tasks[i].0.lower_bound(context.assignments));
    for it in i..tasks.len() {
        let (t, d, _) = tasks[it];
        if it < j { // Omega
            reason.add(predicate![t >= r]);
        } else {
            reason.add(predicate![t >= std::cmp::max(new_lb - sum_p_prime, r)])
        }
        reason.add(predicate![t <= r + sum_p + curr_task_p - 1 - d]);
    }
    reason.add(predicate![curr_task >= r]);
    
    reason
}

fn edge_finding<Var: IntegerVariable + 'static> (
    tasks_init: &Vec<DisjunctiveTask<Var>>,
    context: &mut PropagationContextMut
) -> PropagationStatusCP
{
    let mut tasks = tasks_init.iter().map(|task|
        (task.start_times.clone(), task.duration)
    ).collect::<Vec<_>>();

    tasks.sort_by_key(|(t,d)| -(t.upper_bound(context.assignments) + d));

    let mut tree = ThetaLambdaTree::from_disjunctive(&tasks_init, context.assignments);
    let mut deque: VecDeque<(Var, i32)> = tasks.into_iter().collect();
    let mut j = deque.pop_front().unwrap();
    let mut lct_j;

    while deque.len() > 0 {
        lct_j = j.0.upper_bound(context.assignments) + j.1;
        if tree.ect() > lct_j {
            return compute_overload_explanation(tree, lct_j, context);
        }
        tree.convert_to_gray_id(j.0.domain_id());
        j = deque.pop_front().unwrap();
        lct_j = j.0.upper_bound(context.assignments) + j.1;
        while tree.ect_bar() > lct_j {
            if let Some((id, var, p)) = tree.responsible_ect_bar(0) {
                let new_lb = tree.ect();
                if new_lb > var.lower_bound(context.assignments) {
                    let reason = compute_edge_finding_explanations(&tree, var, p, new_lb, context, lct_j);
                    context.set_lower_bound(var, new_lb, reason)?;
                }
                tree.remove_from_lambda_id(*id);
            } else { break; }
        }
    }
    Ok(())

}

#[cfg(test)]
mod tests {
    use crate::constraints::DisjunctivePropagator;
    use crate::engine::test_solver::TestSolver;

    #[test]
    fn test_propagate_success() {
        let mut solver = TestSolver::default();
        let v1 = solver.new_variable(0, 3); let d1 = 5;
        let v2 = solver.new_variable(2, 4); let d2 = 1;

        let start_times = vec![v1, v2];
        let durations = vec![d1, d2];

        let propagator = solver.new_propagator(
            DisjunctivePropagator::from_vectors(start_times, durations)
        ).expect("fail");

        let result = solver.propagate(propagator);
        assert!(result.is_ok(), "Propagation failed: {:?}", result);
    }

    #[test]
    fn test_propagate_variable_update() {
        let mut solver = TestSolver::default();
        let v1 = solver.new_variable(0, 3); let d1 = 5;
        let v2 = solver.new_variable(2, 4); let d2 = 1;

        let start_times = vec![v1, v2];
        let durations = vec![d1, d2];

        assert_eq!(solver.lower_bound(v1), 0);
        assert_eq!(solver.lower_bound(v2), 2); // Test successful variable creation

        let propagator = solver.new_propagator(
            DisjunctivePropagator::from_vectors(start_times, durations)
        ).expect("fail");

        solver.propagate(propagator).expect("panic");
        assert_eq!(solver.lower_bound(v1), 3);
    }

    #[test]
    fn check_propagate_large() {
        let mut solver = TestSolver::default();
        let t1 = solver.new_variable(0,10);
        let t2 = solver.new_variable(2, 4);
        let t3 = solver.new_variable(11,15);
        let t4 = solver.new_variable(21, 30);
        let t5 = solver.new_variable(24, 25);
        let t6 = solver.new_variable(31, 40);

        let tasks = vec![t1, t2, t3, t4, t5, t6];
        let durations = vec![5,3,10,5,3,20];
        
        let propagator = solver.new_propagator(
            DisjunctivePropagator::from_vectors(tasks, durations)
        ).expect("fail");
        
        let result = solver.propagate(propagator);
        //println!("Propagation result: {:?}", result);
        println!("t1 bounds: {} - {}", solver.lower_bound(t1), solver.upper_bound(t1));
        println!("t2 bounds: {} - {}", solver.lower_bound(t2), solver.upper_bound(t2));
        println!("t3 bounds: {} - {}", solver.lower_bound(t3), solver.upper_bound(t3));
        println!("t4 bounds: {} - {}", solver.lower_bound(t4), solver.upper_bound(t4));
        println!("t5 bounds: {} - {}", solver.lower_bound(t5), solver.upper_bound(t5));
        println!("t6 bounds: {} - {}", solver.lower_bound(t6), solver.upper_bound(t6));
    }
    
    #[test]
    fn check_explanations_failure() {
        let mut solver = TestSolver::default();
        let t1 = solver.new_variable(0,1);
        let t2 = solver.new_variable(6, 11);
        let t3 = solver.new_variable(11,20);
        let t4 = solver.new_variable(12, 16);
        let t5 = solver.new_variable(22, 22);
        
        
        let tasks = vec![t1, t2, t3, t4, t5];
        let durations = vec![6,5,6,5,6];

        let propagator = solver.new_propagator(
            DisjunctivePropagator::from_vectors(tasks, durations)
        );

        println!("t1 bounds: {} - {}", solver.lower_bound(t1), solver.upper_bound(t1));
        println!("t2 bounds: {} - {}", solver.lower_bound(t2), solver.upper_bound(t2));
        println!("t3 bounds: {} - {}", solver.lower_bound(t3), solver.upper_bound(t3));
        println!("t4 bounds: {} - {}", solver.lower_bound(t4), solver.upper_bound(t4));
        println!("t5 bounds: {} - {}", solver.lower_bound(t5), solver.upper_bound(t5));
    }

    #[test]
    fn check_task_bounds() {
        let mut solver = TestSolver::default();
        let x5 = solver.new_variable(81, 88); let d5 = 6;
        let x7 = solver.new_variable(2, 43); let d7 = 4;
        let x8 = solver.new_variable(6, 47); let d8 = 5;
        let x12 = solver.new_variable(3, 73); let d12 = 6;
        let x13 = solver.new_variable(9, 82); let d13 = 3;
        let x14 = solver.new_variable(85, 85); let d14 = 4;
        let x20 = solver.new_variable(19, 90); let d20 = 4;

        let tasks = vec![x5, x7, x8, x12, x13, x14, x20];
        let durations = vec![d5, d7, d8, d12, d13, d14, d20];

        let propagator = solver.new_propagator(
            DisjunctivePropagator::from_vectors(tasks, durations)
        ).expect("fail");

        println!("x5 bounds: {} - {}", solver.lower_bound(x5), solver.upper_bound(x5));
        println!("x7 bounds: {} - {}", solver.lower_bound(x7), solver.upper_bound(x7));
        println!("x8 bounds: {} - {}", solver.lower_bound(x8), solver.upper_bound(x8));
        println!("x12 bounds: {} - {}", solver.lower_bound(x12), solver.upper_bound(x12));
        println!("x13 bounds: {} - {}", solver.lower_bound(x13), solver.upper_bound(x13));
        println!("x14 bounds: {} - {}", solver.lower_bound(x14), solver.upper_bound(x14));
        println!("x20 bounds: {} - {}", solver.lower_bound(x20), solver.upper_bound(x20));
    }
    
    #[test]
    fn custom() {
        let mut solver = TestSolver::default();
        let t1 = solver.new_variable(0,27);
        let t2 = solver.new_variable(21,21);
        let t3 = solver.new_variable(28, 28);
        let t4 = solver.new_variable(25,36);
        
        let tasks = vec![t1, t2, t3, t4];
        let durations = vec![2, 6, 4, 3];
        
        let propagator = solver.new_propagator(
            DisjunctivePropagator::from_vectors(tasks, durations)
        ).expect("fail");
        
        println!("t1 bounds: {} - {}", solver.lower_bound(t1), solver.upper_bound(t1));
        println!("t2 bounds: {} - {}", solver.lower_bound(t2), solver.upper_bound(t2));
        println!("t3 bounds: {} - {}", solver.lower_bound(t3), solver.upper_bound(t3));
        println!("t4 bounds: {} - {}", solver.lower_bound(t4), solver.upper_bound(t4));
    }
}