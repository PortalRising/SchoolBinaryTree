use std::{
    collections::VecDeque,
    fmt::Debug,
    io::{self, Write},
    u64,
};

pub struct Slot<T> {
    item: Option<T>,
}

impl<T> Slot<T> {
    fn new(item: T) -> Self {
        Self { item: Some(item) }
    }

    /// Clear this slot
    fn clear(&mut self) -> Option<T> {
        self.item.take()
    }

    /// Set the value of this slot
    fn set(&mut self, item: T) {
        self.item = Some(item);
    }
}

impl<T> Debug for Slot<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Slot").field("item", &self.item).finish()
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct SlotKey {
    index: usize,
}

impl SlotKey {
    fn new(index: usize) -> Self {
        Self { index }
    }
}

pub struct SlotMap<T> {
    slots: Vec<Slot<T>>,
    item_count: usize,
    /// 1 bit represents full slot and 0 bit represents empty slot
    empty_indexes: Vec<u64>,
}

impl<T> Debug for SlotMap<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlotMap")
            .field("slots", &self.slots)
            .field("item_count", &self.item_count)
            .field("empty_indexes", &self.empty_indexes)
            .finish()
    }
}

impl<T> SlotMap<T> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            item_count: 0,
            empty_indexes: Vec::new(),
        }
    }

    /// Find the next free slot in the SlotMap
    pub fn find_free_slot(&self) -> Option<usize> {
        for (chunk_index, &empty_chunk) in self.empty_indexes.iter().enumerate() {
            let free_slot = empty_chunk.leading_ones();

            if free_slot >= u64::BITS {
                // There are no free slots in this chunk
                continue;
            }

            // There is a free slot at next_slot
            // Apply chunk_index to create the correct offset
            let free_index = (chunk_index * u64::BITS as usize) + free_slot as usize;

            return Some(free_index);
        }

        None
    }

    pub fn insert(&mut self, item: T) -> SlotKey {
        let insert_index: usize;

        // Check if there is an slot we can insert into
        if self.item_count < self.slots.len() {
            // There is a slot somewhere
            let free_index = self.find_free_slot().expect(
                "There must be a free slot, otherwise we are keeping item_count out of sync",
            );

            // Update the slot to store this item
            self.slots[free_index].set(item);

            insert_index = free_index;
        } else {
            // Just insert the item, as there is no open space
            let slot = Slot::new(item);

            insert_index = self.slots.len();

            self.slots.push(slot);
        }

        // Increment item_count
        self.item_count += 1;

        // Create an index referring to this item
        SlotKey::new(insert_index)
    }

    /// Remove an item from the SlotMap
    pub fn remove(&mut self, slot_key: SlotKey) -> T {
        let slot = self
            .slots
            .get_mut(slot_key.index)
            .expect("Index should be in range as SlotMap never shrinks");

        self.item_count -= 1;

        // We now need to set a free bit, but if the chunks have not been generated we must generate them
        let slot_chunk = slot_key.index / u64::BITS as usize;
        while (slot_chunk + 1) - self.empty_indexes.len() > 0 {
            // If we haven't removed any elements from this chunk of elements then it must all be full
            // or outside the range of the SlotMap
            self.empty_indexes.push(u64::MAX);
        }

        // Convert the index into a bit offset
        let bit_length = u64::BITS as usize;
        let bit_offset = bit_length - 1 - (slot_key.index % bit_length);
        // Locate the bit we must unset
        let slot_mask = 1_u64 << bit_offset;
        // Invert the mask so we can use AND to unset the bit
        let unset_mask = !slot_mask;

        self.empty_indexes[slot_chunk] &= unset_mask;

        // This should never return None, as the generation index matched
        // but thats for the caller to handle
        slot.clear().expect("Key exists so should data")
    }

    /// Get a reference to an item from the SlotMap
    pub fn get(&self, slot_key: SlotKey) -> &T {
        let slot = self
            .slots
            .get(slot_key.index)
            .expect("Index should be in range as SlotMap never decreases in length");

        // This should never return None, as the key cannot be copied and the key only gets used on removal
        slot.item.as_ref().expect("Should exist as key exists")
    }

    /// Get a mutable reference to an item from the SlotMap
    pub fn get_mut(&mut self, slot_key: SlotKey) -> &mut T {
        let slot = self
            .slots
            .get_mut(slot_key.index)
            .expect("Index should be in range as SlotMap never decreases in length");

        // This should also never return None, for the reason above
        slot.item.as_mut().expect("Should exist as key exists")
    }
}

pub struct TreeNode<T> {
    data: T,
    left: Option<SlotKey>,
    right: Option<SlotKey>,
}

impl<T> Debug for TreeNode<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TreeNode")
            .field("data", &self.data)
            .field("left", &self.left)
            .field("right", &self.right)
            .finish()
    }
}

impl<T> TreeNode<T> {
    fn new(data: T) -> Self {
        Self {
            data,
            left: None,
            right: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum TreeDirection {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy)]
pub enum TreeOrdering {
    /// NLR
    Pre,
    /// LNR
    In,
    /// LRN
    Post,
}

pub struct Tree<T> {
    storage: SlotMap<TreeNode<T>>,
    root: SlotKey,
}

impl<T> Tree<T> {
    pub fn new(root: T) -> Self {
        let mut storage = SlotMap::new();

        let root = storage.insert(TreeNode::new(root));

        Self { storage, root }
    }
}

impl<T> Debug for Tree<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tree")
            .field("storage", &self.storage)
            .field("root", &self.root)
            .finish()
    }
}

impl<T> Tree<T>
where
    T: Eq + Ord + Debug,
{
    /// Insert data into the tree, placing it in an ordered location
    ///
    /// # NOTE
    ///
    /// If the data already exists in the tree, then it just returns the data
    pub fn insert_ordered(&mut self, data: T) -> Result<(), T> {
        // Store the current node we are viewing
        let mut current_key = self.root;
        let insert_direction: TreeDirection;

        // Locate the location to insert into
        loop {
            let current_node = self.storage.get(current_key);

            if data == current_node.data {
                // We cannot accept duplicates
                return Err(data);
            }

            if data < current_node.data {
                // Data is smaller so we need to descend the
                // left path

                if let Some(left_node) = current_node.left {
                    current_key = left_node;
                    continue;
                }

                // This is a leaf node for the left side
                // Insert here
                insert_direction = TreeDirection::Left;
                break;
            } else {
                // Data is greater so we look through the right

                if let Some(right_node) = current_node.right {
                    current_key = right_node;
                    continue;
                }

                // This is a leaf node for the right side
                // Insert here
                insert_direction = TreeDirection::Right;
                break;
            }
        }

        // Create a new node with our data
        let new_node = self.storage.insert(TreeNode::new(data));

        // Get the last node mutably
        let insert_node = self.storage.get_mut(current_key);

        match insert_direction {
            TreeDirection::Left => {
                insert_node.left = Some(new_node);
            }
            TreeDirection::Right => {
                insert_node.right = Some(new_node);
            }
        }

        Ok(())
    }

    /// Check if tree contains
    pub fn contains(&self, data: &T) -> bool {
        // Store the current node we are viewing
        let mut current_key = self.root;

        // Locate the location to insert into
        loop {
            let current_node = self.storage.get(current_key);

            if *data == current_node.data {
                // It exists
                return true;
            }

            if *data < current_node.data {
                // Data is smaller so we need to descend the
                // left path

                if let Some(left_node) = current_node.left {
                    current_key = left_node;
                    continue;
                }

                // This is a leaf node for the left side
                // It must not exist then
                return false;
            } else {
                // Data is greater so we look through the right

                if let Some(right_node) = current_node.right {
                    current_key = right_node;
                    continue;
                }

                // This is a leaf node for the right side
                // It must not exist then
                return false;
            }
        }
    }

    /// Deletes an element if it exists
    pub fn delete(self, data: &T) -> Self {
        if !self.contains(data) {
            // Does not contain the data so do nothing,
            return self;
        }

        let slots = self.storage.slots;

        let mut items = slots
            .into_iter()
            .filter_map(|slot| slot.item.map(|node| node.data))
            .filter(|slot_data| slot_data != data);

        let root = items.next().expect("All trees must have one root");

        let mut new_tree = Tree::new(root);

        for item in items.into_iter() {
            new_tree
                .insert_ordered(item)
                .expect("Data should be unique as it is in the tree");
        }

        new_tree
    }

    /// Print the tree an order provided
    pub fn out_order(&self, ordering: TreeOrdering) {
        // LNR

        println!("-- {:?} order start: ", ordering);

        self.inner_out(ordering, Some(self.root));

        println!("-- {:?} order end", ordering);
    }

    fn inner_out(&self, ordering: TreeOrdering, node_key: Option<SlotKey>) {
        let node = if let Some(node_key) = node_key {
            self.storage.get(node_key)
        } else {
            return;
        };

        match ordering {
            TreeOrdering::Pre => {
                println!("{:?}", node.data);

                self.inner_out(ordering, node.left);

                self.inner_out(ordering, node.right);
            }
            TreeOrdering::In => {
                self.inner_out(ordering, node.left);

                println!("{:?}", node.data);

                self.inner_out(ordering, node.right);
            }
            TreeOrdering::Post => {
                self.inner_out(ordering, node.left);

                self.inner_out(ordering, node.right);

                println!("{:?}", node.data);
            }
        }
    }

    /// Print the tree, breadth first
    pub fn out_breadth(&self) {
        let mut queue = VecDeque::<SlotKey>::new();
        queue.push_back(self.root);

        println!("-- Breadth order start: ");

        while !queue.is_empty() {
            let current_key = queue.pop_front().expect("Queue is not empty");
            let current_node = self.storage.get(current_key);

            println!("{:?}", current_node.data);

            // Insert the left node if it exists
            if let Some(left_node) = current_node.left {
                queue.push_back(left_node);
            }

            // Insert the right node if it exists
            if let Some(right_node) = current_node.right {
                queue.push_back(right_node);
            }
        }

        println!("-- Breadth order end ");
    }
}

fn read_node(prompt: &str) -> String {
    print!("{}", prompt);
    // Make sure we can see the text
    io::stdout().flush().expect("Flushing should work");

    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .expect("Should read from STDIN fine");

    line.trim_end().to_owned()
}

fn user_tree(root: String) -> Tree<String> {
    let mut tree = Tree::new(root);

    // Allow the user to add as many nodes as they like until END
    loop {
        let new_node = read_node("Please enter new node or END to stop: ");

        if new_node.trim().to_lowercase() == "end" {
            // No more nodes
            break;
        }

        if let Err(_) = tree.insert_ordered(new_node) {
            println!("Data already in tree...");
        }
    }

    tree
}

fn main() {
    // Force the user to enter a root node
    let root = read_node("Please enter root node or 'AUTO' for automatic testing: ");

    let mut tree = if root == "AUTO" {
        let nodes = [
            "X", "D", "U", "G", "V", "O", "P", "I", "C", "S", "W", "Y", "F", "A",
        ];

        let mut tree = Tree::new("H".to_owned());

        for node in nodes.map(|i| i.to_owned()) {
            tree.insert_ordered(node).expect("Letters are unique");
        }

        tree
    } else {
        user_tree(root)
    };

    let orderings = [TreeOrdering::Pre, TreeOrdering::In, TreeOrdering::Post];

    for order in orderings {
        tree.out_order(order);
    }

    tree.out_breadth();

    // Check if the tree contains P
    let p = "P".to_owned();

    println!("Does the tree contain P? {}", tree.contains(&p));

    // Delete P then check again
    println!("Deleting P");
    tree = tree.delete(&p);

    println!("Does the tree contain P? {}", tree.contains(&p));
}
