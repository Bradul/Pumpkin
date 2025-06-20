use std::collections::VecDeque;
use std::fmt::Debug;
use crate::constraints::DisjunctiveTask;
use crate::engine::Assignments;
use crate::engine::propagation::{PropagationContext, PropagationContextMut};
use crate::pumpkin_assert_simple;
use crate::variables::{DomainId, IntegerVariable};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Node<Var: IntegerVariable + 'static> {
    ect: i32,
    sum_p: i32,
    ect_bar: i32,
    sum_p_bar: i32,
    node_id: Option<(DomainId, Var)>
}

impl<Var: IntegerVariable + 'static> Node<Var> {
    fn new() -> Self {
        Self {
            ect: i32::MIN,
            sum_p: 0,
            ect_bar: i32::MIN,
            sum_p_bar: 0,
            node_id: None,
        }
    }

    fn white_node(ect: i32, p: i32) -> Self {
        Self {
            ect,
            sum_p: p,
            ect_bar: ect,
            sum_p_bar: p,
            node_id: None,
        }
    }

    fn to_gray(&mut self) {
        self.ect = i32::MIN;
        self.sum_p = 0;
    }

    fn to_empty(&mut self) {
        self.ect = i32::MIN;
        self.sum_p = 0;
        self.ect_bar = i32::MIN;
        self.sum_p_bar = 0;
    }

    fn is_leaf(&self) -> bool {
        self.ect_bar != i32::MIN
    }

    fn is_gray(&self) -> bool {
        self.ect == i32::MIN && self.is_leaf()
    }
}

#[derive(Debug)]
pub(super) struct ThetaLambdaTree<Var: IntegerVariable + 'static> {
    pub(super) nodes: Vec<Node<Var>>,
    n: usize,
    num_tasks: usize,
}

impl<Var: IntegerVariable + 'static> ThetaLambdaTree<Var> {
    pub(super) fn from_vectors(tasks: Vec<Var>, durations: Vec<i32>, assignment: &mut Assignments) -> Self {
        let zipped = tasks
            .into_iter()
            .zip(durations.into_iter())
            .collect::<Vec<_>>();

        Self::new(zipped, assignment)
    }

    pub(super) fn from_disjunctive(tasks: &Vec<DisjunctiveTask<Var>>, assignment: &mut Assignments) -> Self {
        let cloned_tasks = tasks.into_iter()
            .map(|task| (task.start_times.clone(), task.duration))
            .collect::<Vec<_>>();
        Self::new(cloned_tasks, assignment)
    }

    fn new(mut zipped: Vec<(Var, i32)>, assignment: &mut Assignments) -> Self {
        zipped.sort_by_key(|task| task.0.lower_bound(assignment));

        let n = zipped.len();
        let next_p2 = n.next_power_of_two();

        let mut nodes = vec![Node::new(); 2 * next_p2 - 1];

        for i in 0..n {
            let (task, duration) = &zipped[i];
            let ect = task.lower_bound(assignment) + duration;
            nodes[next_p2+i-1] = Node::white_node(ect, *duration);
            nodes[next_p2+i-1].node_id = Some((task.domain_id(), task.clone()));
        }

        let mut tree = Self {
            nodes,
            n: next_p2 - 1,
            num_tasks: n
        };
        for i in (0..tree.n).rev() {
            tree.update_node(i);
        }
        tree
    }

    pub(super) fn get_left_child(&self, i: usize) -> usize {
        2 * i + 1
    }

    pub(super) fn get_right_child(&self, i: usize) -> usize {
        2 * i + 2
    }

    pub(super) fn get_parent(&self, i: usize) -> usize {
        (i - 1) / 2
    }

    pub(super) fn find_id(&self, id: DomainId) -> Option<usize> {
        for i in self.n..self.nodes.len() {
            match self.nodes[i].node_id {
                None => {continue}
                Some((did, _)) => {
                    if did == id {
                        return Some(i);
                    }
                }
            }
        }
        None
    }
    
    pub(super) fn est(&self, context: PropagationContext) -> i32 {
        *(&self.nodes[self.n].node_id.as_ref().unwrap().1.lower_bound(context.assignments))
    }

    pub(super) fn ect(&self) -> i32 {
        self.nodes[0].ect
    }

    pub(super) fn ect_bar(&self) -> i32 {
        self.nodes[0].ect_bar
    }

    pub(super) fn sum_p(&self) -> i32 {
        self.nodes[0].sum_p
    }

    pub(super) fn sum_p_bar(&self) -> i32 {
        self.nodes[0].sum_p_bar
    }

    pub(super) fn update_node(&mut self, index: usize) {
        let left = self.get_left_child(index);
        let right = self.get_right_child(index);

        if left < self.nodes.len() {
            self.nodes[index].ect = self.nodes[right].ect.max(self.nodes[left].ect + self.nodes[right].sum_p);
            self.nodes[index].sum_p = self.nodes[left].sum_p + self.nodes[right].sum_p;
            self.nodes[index].sum_p_bar = std::cmp::max(self.nodes[left].sum_p_bar + self.nodes[right].sum_p, self.nodes[left].sum_p + self.nodes[right].sum_p_bar);
            self.nodes[index].ect_bar = std::cmp::max(
                self.nodes[right].ect_bar,
                std::cmp::max(
                    self.nodes[left].ect + self.nodes[right].sum_p_bar,
                    self.nodes[left].ect_bar + self.nodes[right].sum_p,
                )
            )
        }
    }

    pub(super) fn update_node_along_path(&mut self, index: usize) {
        let mut i = index;
        while i > 0 {
            i = self.get_parent(i);
            self.update_node(i);
        }
    }

    pub(super) fn responsible_sum_p_bar(&self, index: usize) -> Option<(&DomainId, &Var, i32)> {
        if index >= self.n { // Leaf node
            if self.nodes[index].is_gray() { // Gray leaf
                Some(&self.nodes[index].node_id.as_ref().unwrap()).map(|(id, var)| (id, var, self.nodes[index].sum_p_bar))
            } else {None}
        } else { // Internal node
            let left = self.get_left_child(index);
            let right = self.get_right_child(index);
            if self.nodes[index].sum_p_bar == self.nodes[left].sum_p_bar + self.nodes[right].sum_p {
                self.responsible_sum_p_bar(left)
            } else if self.nodes[index].sum_p_bar == self.nodes[left].sum_p + self.nodes[right].sum_p_bar {
                self.responsible_sum_p_bar(right)
            } else {None}
        }
    }

    pub(super) fn responsible_ect_bar(&self, index: usize) -> Option<(&DomainId, &Var, i32)> {
        if index >= self.n { // Leaf node
            if self.nodes[index].is_gray() { // Gray leaf, and not removed
                Some(&self.nodes[index].node_id.as_ref().unwrap()).map(|(id, var)| (id, var, self.nodes[index].sum_p_bar))
            } else {None}
        } else {
            let left = self.get_left_child(index);
            let right = self.get_right_child(index);
            if self.nodes[index].ect_bar == self.nodes[right].ect_bar {
                self.responsible_ect_bar(right)
            } else if self.nodes[index].ect_bar == self.nodes[left].ect + self.nodes[right].sum_p_bar {
                self.responsible_sum_p_bar(right)
            } else if self.nodes[index].ect_bar == self.nodes[left].ect_bar + self.nodes[right].sum_p {
                self.responsible_ect_bar(left)
            } else {None}
        }
    }

    pub(super) fn convert_to_gray_index(&mut self, index: usize) {
        pumpkin_assert_simple!(index >= self.n);
        pumpkin_assert_simple!(index < self.nodes.len());

        self.nodes[index].to_gray();

        self.update_node_along_path(index);
    }

    pub(super) fn convert_to_gray_id(&mut self, id: DomainId) {
        let index = self.find_id(id).expect("Node not found");
        self.convert_to_gray_index(index);
    }

    pub(super) fn remove_from_lambda_id(&mut self, id: DomainId) {
        let index = self.find_id(id).expect("Node not found");
        self.nodes[index].to_empty();
        self.update_node_along_path(index);
    }

    /*pub(super) fn rearrange_updated_node(&mut self, id: DomainId, assignment: &Assignments) {
        let index = self.find_id(id).expect("Node not found");
        //Update leaf first
        if self.nodes[index].ect == i32::MIN { // Node is gray - only update ect_bar
            self.nodes[index].ect_bar = id.lower_bound(assignment) + self.nodes[index].sum_p_bar;
        } else { // Update ect and ect_bar
            self.nodes[index].ect = id.lower_bound(assignment) + self.nodes[index].sum_p;
            self.nodes[index].ect_bar = id.lower_bound(assignment) + self.nodes[index].sum_p;
        }

        if let Some(pos) = self.sorted_indices.iter().position(|&x| x == index) {
            // In most cases, rearranging is not necessary. We can do a binary search to check whether it is, to avoid adding/removing and updating unnecessarily.
            if pos < self.sorted_indices.len() - 1 {
                let next_lb = self.nodes[self.sorted_indices[pos + 1]].node_id.unwrap().lower_bound(assignment);
                if id.lower_bound(assignment) <= next_lb { // No need to go further. Just upwards update from the current node.
                    self.update_node_along_path(index);
                    return;
                }
            }

            // If reordering is necessary, proceed with moving the node.
            let _ = self.sorted_indices.remove(pos);

            let est = id.lower_bound(assignment);
            let insert_pos = self.sorted_indices.binary_search_by_key(&est, |&i| {
                self.nodes[i].node_id.unwrap().lower_bound(assignment)
            }).unwrap_or_else(|i| i);

            self.sorted_indices.insert(insert_pos, index);

            let mut update_queue: VecDeque<usize> = VecDeque::new();
            let remove_pos = if pos % 2 == 1 {pos - 1} else {pos};
            for x in (remove_pos..(insert_pos+1)).step_by(2) {
                update_queue.push_back((x + self.n) / 2); // Add directly the parents of affected nodes.
            }
            // Variant 2: Update nodes one by one, not in pairs of two.
            // More conceptually sound, due to it being guaranteed to reach the parent, if updates happen.
            while update_queue.len() > 0 {
                let x = update_queue.pop_front().unwrap();
                if x != 0 {
                    let parent_x = self.get_parent(x);
                    if parent_x != *update_queue.front().unwrap() { // No duplicate parents in the queue
                        update_queue.push_back(parent_x);
                    }
                }
                self.update_node(x);
            }
        }
    } Thought I needed this, but removal is simpler. Will keep it for now, in case I need it later.
    */
    
    // Methods for Overload Explanations

    pub(super) fn get_all_nodes(&self) -> Vec<(&Var, i32, bool)> {
        // Bool is true if node is in theta, false if in lambda/removed.
        let mut nodes = Vec::new();
        for i in self.n..self.nodes.len() {
            if self.nodes[i].ect != i32::MIN {
                nodes.push((
                    &self.nodes[i].node_id.as_ref().unwrap().1,
                    self.nodes[i].sum_p,
                    true
                ))
            } else {
                match &self.nodes[i].node_id {
                    Some(id) => {
                        nodes.push((&id.1, self.nodes[i].sum_p, false))
                    }
                    None => continue
                }
            }
        }
        nodes
    }

    pub(super) fn get_all_theta(&self) -> Vec<(&Var, i32, bool)> {
        let mut theta_ids = Vec::new();
        for i in self.n..self.nodes.len() {
            let index = i;
            if self.nodes[index].ect != i32::MIN {
                theta_ids.push((
                    &self.nodes[index].node_id.as_ref().unwrap().1,
                    self.nodes[index].sum_p,
                    true
                ));
            }
        }
        theta_ids
    }
    
    // Methods for Edge-finding Explanations
    
    // For efficiency, returns a vector of both omega and omega' sets. Boolean indicates if the node is in omega' (true) or omega (false).
    pub(super) fn get_all_omegas(&self, index: usize) -> (Vec<(&Var, i32)>, i32) {
        if index >= self.n {
            if self.nodes[index].ect == i32::MIN {
                return (vec![], 0);
            }
            return (vec![(
                &self.nodes[index].node_id.as_ref().unwrap().1,
                self.nodes[index].sum_p,
            )], self.nodes[index].sum_p)
        }
        
        let left = self.get_left_child(index);
        let right = self.get_right_child(index);
        if self.nodes[left].ect + self.nodes[right].sum_p == self.nodes[index].ect {
            let (mut left_omegas, sp) = self.get_all_omegas(left);
            left_omegas.extend(self.get_all_leaves(right));
            (left_omegas, sp + self.nodes[right].sum_p)
        } else {
            self.get_all_omegas(right)
        }
    }
    
    pub(super) fn get_all_leaves(&self, index: usize) -> Vec<(&Var, i32)> {
        if index >= self.n {
            if self.nodes[index].ect == i32::MIN {
                return vec![];
            }
            return vec![(
                &self.nodes[index].node_id.as_ref().unwrap().1,
                self.nodes[index].sum_p,
            )]
        }
        
        let left = self.get_left_child(index);
        let right = self.get_right_child(index);
        let mut left_leaves = self.get_all_leaves(left);
        left_leaves.extend(self.get_all_leaves(right));
        left_leaves
    }
}

#[cfg(test)]
mod tests {
    use crate::constraints::theta_lambda_trees::ThetaLambdaTree;
    use crate::engine::test_solver::TestSolver;
    use crate::variables::IntegerVariable;

    #[test]
    fn test_tree_example_vilim() {
        let mut solver = TestSolver::default();
        let a = solver.new_variable(0,0);
        let b = solver.new_variable(25, 25);
        let c = solver.new_variable(30, 30);
        let d = solver.new_variable(32, 32);

        let tasks = vec![a, b, c, d];
        let durations = vec![5, 9, 5, 10];

        let mut tree = ThetaLambdaTree::from_vectors(tasks, durations, &mut solver.assignments);

        tree.convert_to_gray_index(5);

        assert_eq!(tree.ect(), 44);
        assert_eq!(tree.ect_bar(), 49);
    }

    #[test]
    fn test_tree_my_test() {
        let mut solver = TestSolver::default();
        let t1 = solver.new_variable(0,10);
        let t2 = solver.new_variable(2, 4); // gray
        let t3 = solver.new_variable(11,15);
        let t4 = solver.new_variable(21, 30);
        let t5 = solver.new_variable(25,26); // gray
        let t6 = solver.new_variable(31, 40); // gray

        let tasks = vec![t1, t2, t3, t4, t5, t6];
        let durations = vec![5,2,10,5,1,20];

        let mut tree = ThetaLambdaTree::from_vectors(tasks, durations, &mut solver.assignments);

        tree.convert_to_gray_id(t2);
        tree.convert_to_gray_id(t5);
        tree.convert_to_gray_id(t6);

        assert_eq!(tree.ect_bar(), 51);
        assert_eq!(tree.sum_p_bar(), 40);
        assert_eq!(tree.ect(), 26);
        assert_eq!(tree.sum_p(), 20);
    }

    #[test]
    fn test_one_node() {
        let mut solver = TestSolver::default();
        let t1 = solver.new_variable(0,10);
        let tasks = vec![t1];
        let durations = vec![5];

        let tree = ThetaLambdaTree::from_vectors(tasks, durations, &mut solver.assignments);

        println!("[{:?}]", tree);
        assert_eq!(tree.ect(), 5);
        assert_eq!(tree.ect_bar(), 5);
    }

    #[test]
    fn test_something() {
        let mut solver = TestSolver::default();
        let t1 = solver.new_variable(0,10);
        let t2 = solver.new_variable(2, 4);
        let t3 = solver.new_variable(11,15);

        let tasks = vec![t1, t2, t3];
        let durations = vec![5,3,10];

        let tree = ThetaLambdaTree::from_vectors(tasks, durations, &mut solver.assignments);

        println!("[{:?}]", tree.nodes[0]);
        println!("[{:?}]", tree.nodes[1]);
        println!("[{:?}]", tree.nodes[2]);
        println!("[{:?}]", tree.nodes[3]);
        println!("[{:?}]", tree.nodes[4]);
    }
}