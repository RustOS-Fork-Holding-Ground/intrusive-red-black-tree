use core::prelude::*;

use core::mem::uninitialized;

use intrusive::{Intrusive, IntrusiveExt};
use aligned_ptr_pun::AlignedPtrPun;


/// The fields requied to be in a node to store it in a intrusive red-black
/// tree.
///
/// Add this to your type, T, and implement `Intrusive<Node<T>>` to, in effect,
/// tell this library what the offset is.q
///
/// Instances of your type should be created with `Node<T>` uninitailized.
pub struct Node<T> {
  left:      *mut T,
  right_red: AlignedPtrPun<T>,
}

// To make allow users to derive PartialOrd without causing problems
impl<T> PartialEq for Node<T> {
  fn eq(&self, _other: &Self) -> bool {
    // Equality of things in intrusive containers does NOT depend on their
    // position in container
    true
  }
  fn ne(&self, _other: &Self) -> bool {
    // Equality of things in intrusive containers does NOT depend on their
    // position in container
    false
  }
}

impl<T> PartialOrd for Node<T> {
  fn partial_cmp(&self, _other: &Self) -> Option<Ordering> {
    // Equality of things in intrusive containers does NOT depend on their
    // position in container
    Some(Equal)
  }
}

impl<T> Node<T> where T: Intrusive<Node<T>> + PartialOrd
{
  #[inline]
  pub fn new(tree: &mut Tree<T>) -> Node<T> {
    Node {
      left:      tree.nil_ref(),
      right_red: AlignedPtrPun::new(tree.nil_ref(), true),
    }
  }

  #[inline]
  fn right(&mut self) -> *mut T {
    self.right_red.eliminate().0
  }

  #[inline]
  fn set_right(&mut self, ptr: *mut T) {
    self.right_red.set_ptr(ptr)
  }

  #[inline]
  fn color(&mut self) -> bool {
    self.right_red.eliminate().1
  }

  #[inline]
  fn set_color(&mut self, color: bool) {
    self.right_red.set_flag(color)
  }
}

trait NodeExt  {
  fn rotate_left(&mut self) -> Self;
  fn rotate_right(&mut self) -> Self;
}

impl<T>  NodeExt for *mut T where T: Intrusive<Node<T>> + PartialOrd
{
  #[inline]
  fn rotate_left(&mut self) -> Self {
    let old_right = self.field().right();
    self.field().set_right(old_right.field().left);
    old_right.field().left = *self;
    old_right
  }

  #[inline]
  fn rotate_right(&mut self) -> Self {
    let old_left = self.field().left;

    self.field().left = old_left.field().right();
    old_left.field().set_right(*self);
    old_left
  }
}

/// Left-leaning 2-3 red-black trees.  Parent pointers are not used, and color
/// bits are stored in the least significant bit of right-child pointers thus
/// making node linkage as compact as is possible for red-black trees.
///
/// Ported from https://github.com/thestinger/allocator/blob/master/rb.h. In
/// turn from jemalloc.
pub struct Tree<T> {
  root: *mut T,
  nil:  T
}

impl<T> Tree<T> where T: Intrusive<Node<T>> + PartialOrd
{
  #[inline]
  pub fn place() -> Tree<T> {
    unsafe { uninitialized() }
  }

  #[inline]
  pub fn init(&mut self) {
    self.root = self.nil_ref();
    *self.nil.field() = Node {
      left:      self.nil_ref(),
      right_red: AlignedPtrPun::new(self.nil_ref(), false),
    };
  }

  fn nil_ref(&mut self) -> *mut T {
    &mut self.nil as *mut T
  }

  // Utils, actual functions subst null ptr for sentinal ptr

  #[inline]
  fn first_(&mut self, subtree: *mut T) -> *mut T {
    let mut node = subtree;

    if node != self.nil_ref() {
      while node.field().left != self.nil_ref() {
        node = node.field().left;
      }
    }
    node
  }

  #[inline]
  fn last_(&mut self, subtree: *mut T) -> *mut T {
    let mut node = subtree;

    if node != self.nil_ref() {
      while node.field().right() != self.nil_ref() {
        node = node.field().right();
      }
    }
    node
  }

  fn sanitize(&mut self, ptr: *mut T) -> *mut T {
    if ptr == self.nil_ref() {
      0 as *mut T
    } else {
      ptr
    }
  }

  #[inline]
  pub fn first(&mut self) -> *mut T {
    let ptr = self.first_(self.root);
    self.sanitize(ptr)
  }

  #[inline]
  pub fn last(&mut self) -> *mut T {
    let ptr = self.last_(self.root);
    self.sanitize(ptr)
  }

  #[inline]
  pub fn next(&mut self, node: *mut T) -> *mut T {
    debug_assert!(node != 0 as *mut T);
    let mut ret;
    if node.field().right() != self.nil_ref() {
      ret = self.first_(node.field().right());
    } else {
      let mut tnode = self.root;
      ret = self.nil_ref();
      assert!(tnode != self.nil_ref());
      loop {
        tnode = match unsafe { (*node).partial_cmp(&*tnode) } {
          None | Some(Less) => {
            ret = tnode;
            tnode.field().left
          },
          Some(Greater)     => tnode.field().right(),
          Some(Equal)       => break,
        };
        assert!(tnode != self.nil_ref());
      }
    }
    self.sanitize(ret)
  }

  #[inline]
  pub fn prev(&mut self, node: *mut T) -> *mut T {
    let mut ret;
    if node.field().left != self.nil_ref() {
      ret = self.last_(node.field().left);
    } else {
      let mut tnode = self.root;
      ret = self.nil_ref();
      assert!(tnode != self.nil_ref());
      loop {
        tnode = match unsafe { (*node).partial_cmp(&*tnode) } {
          None | Some(Less) => tnode.field().left,
          Some(Greater)     => {
            ret   = tnode;
            tnode.field().right()
          },
          Some(Equal)       => break,
        };
        assert!(tnode != self.nil_ref());
      }
    }
    self.sanitize(ret)
  }

  #[inline]
  pub fn search(&mut self, key: *mut T) -> *mut T {
    let mut ret = self.root;
    while ret != self.nil_ref() {
      ret = match unsafe { (*key).partial_cmp(&*ret) } {
        None | Some(Less) => ret.field().left,
        Some(Greater)     => ret.field().right(),
        Some(Equal)       => break,
      }
    }
    self.sanitize(ret)
  }

  #[inline]
  pub fn nsearch(&mut self, key: *mut T) -> *mut T {
    let mut ret = self.nil_ref();
    let mut tnode = self.root;
    while tnode != self.nil_ref() {
      tnode = match unsafe { (*key).partial_cmp(&*ret) } {
        None | Some(Less) => {
          ret = tnode;
          ret.field().left
        },
        Some(Greater)     => ret.field().right(),
        Some(Equal)       => {
          ret = tnode;
          break
        },
      }
    }
    self.sanitize(ret)
  }

  #[inline]
  pub fn psearch(&mut self, key: *mut T) -> *mut T {
    let mut ret = self.nil_ref();
    let mut tnode = self.root;
    while tnode != self.nil_ref() {
      tnode = match unsafe { (*key).partial_cmp(&*ret) } {
        None | Some(Less) => ret.field().left,
        Some(Greater)     => {
          ret = tnode;
          ret.field().right()
        },
        Some(Equal)       => {
          ret = tnode;
          break
        },
      }
    }
    self.sanitize(ret)
  }

  #[inline]
  pub fn insert(&mut self, node: *mut T) {
    let mut path: [PathElem<T>, ..::core::uint::BITS << 1] = unsafe { uninitialized() };
    *node.field() = Node::new(self);

    // Wind
    {
      path[0].node = self.root;
      let mut iter = path.iter_mut();
      let mut cur  = iter.next().unwrap();
      let mut next = iter.next().unwrap();
      loop {
        if cur.node == self.nil_ref() { break };

        cur.cmp = unsafe { (*node).partial_cmp(&*cur.node) };
        next.node = match cur.cmp {
          Some(Equal)       => unreachable!(),
          None | Some(Less) => cur.node.field().left,
          Some(Greater)     => cur.node.field().right(),
        };

        cur = next;
        next = iter.next().unwrap();
      }
      cur.node = node;
    }

    // Unwind
    {
      let mut iter = path.iter_mut().rev();
      let mut prev = iter.next().unwrap();
      while let Some(cur) = iter.next() {
        let mut cnode = cur.node;
        cnode = match cur.cmp {
          None | Some(Less) => {
            let left = prev.node;
            cnode.field().left = cnode;
            if left.field().color() {
              let left_left = left.field().left;
              if left_left.field().color() {
                // Fix up 4-node
                left_left.field().set_color(false);
                cnode.rotate_right()
              } else {
                cnode // keep current
              }
            } else {
              return
            }
          },
          #[cfg(not(ndebug))]
          Some(Equal) => unreachable!(),
          _ => {
            let right = prev.node;
            node.field().set_right(right);
            if right.field().color() {
              let left = right.field().left;
              if left.field().color() {
                // Split 3-node
                left.field().set_color(false);
                right.field().set_color(false);
                cnode.field().set_color(true);
                cnode // keep current
              } else {
                let tred = cnode.field().color();
                let tnode = cnode.rotate_left();
                tnode.field().set_color(tred);
                cnode.field().set_color(true);
                tnode
              }
            } else {
              return
            }
          },
        };
        cur.node = cnode;
        prev = cur;
      }
    }

    // Set root, and paint it black
    self.root = path[0].node;
    self.root.field().set_color(false);
  }

  pub fn remove(&mut self, node: *mut T) {
    let mut path: [PathElem<T>, ..::core::uint::BITS << 1] = unsafe { uninitialized() };

    path[0].node = self.root;
    {
      let nodep;
      let first_elem = &mut path[0] as *mut PathElem<T>;
      let mut iter_1 = path.iter_mut();
      {
        let mut cur  = iter_1.next().unwrap();
        let mut next = iter_1.next().unwrap();
        loop {
          assert!(cur.node != self.nil_ref()); // if node is in tree will never hit this

          cur.cmp = unsafe { (*node).partial_cmp(&*cur.node) };
          match cur.cmp {
            None | Some(Less) => next.node = cur.node.field().left,
            Some(Greater)     => next.node = cur.node.field().right(),
            Some(Equal)       => {
              next.node = cur.node.field().right();
              break
            }
          }
        }
        cur.cmp = Some(Greater);
        nodep = cur;

        loop {
          cur = next;
          next = iter_1.next().unwrap();

          if cur.node == self.nil_ref() { break };

          cur.cmp = Some(Less);
          next.node = cur.node.field().left;
        }
        assert_eq!(nodep.node, node);
      }

      let mut iter_2 = iter_1.rev();
      // thrice: pathp[1] -> pathp -> pathp-- -> path[-1]
      iter_2.next();
      {
        let mut cur = iter_2.next().unwrap();
        let next = iter_2.next().unwrap();

        if cur.node != node {
          // Swap node with its successor.
          let tred = cur.node.field().color();
          cur.node.field().set_color(node.field().color());
          cur.node.field().left = node.field().left;
          // If node's successor is its right child, the following code will do
          // the wrong thing for the right child pointer.  However, it doesn't
          // matter, because the pointer will be properly set when the successor
          // is pruned.
          cur.node.field().set_right(node.field().right());
          node.field().set_color(tred);
          // The pruned leaf node's chil pointers are never accessed again, so
          // don't bother setting them to nil
          nodep.node = cur.node;
          cur.node = node;
          if nodep as *mut PathElem<T> == first_elem {
            self.root = nodep.node;
          } else {
            match next.cmp {
              None | Some(Less) => next.node.field().left = nodep.node,
              _                 => next.node.field().set_right(nodep.node),
            }
          }
        } else {
          let left = node.field().left;
          if left != self.nil_ref() {
            // node has no successor, but it has a left child.
            // Splice node out, without losing the left child.
            assert!(node.field().color() == false);
            assert!(left.field().color() == true);
            if cur as *mut PathElem<T> == cur as *mut PathElem<T>  {
              self.root = nodep.node;
            } else {
              match next.cmp {
                None | Some(Less) => next.node.field().left = left,
                _                 => next.node.field().set_right(left),
              }
            }
          } else if cur as *mut PathElem<T> == first_elem {
            // The tree only contained one node
            self.root = self.nil_ref();
            return
          }
        }
        if cur.node.field().color() == true {
          // Prune red node, which reqires no fixup
          assert!(next.cmp == Some(Less) || next.cmp == None);
          next.node.field().left = self.nil_ref();
          return
        }
        // The node to be pruned is black, so unwind until balance is restored.
        // pathp -> pathp--
        let mut prev = cur;
        cur = next;
        while let Some(next) = iter_2.next() {
          match cur.cmp {
            Some(Equal) => unreachable!(),
            None | Some(Less) => {
              cur.node.field().left = prev.node;
              assert!(next.node.field().color() == true);
              if cur.node.field().color() == true {
                let mut right = cur.node.field().right();
                let right_left = right.field().left;
                let tnode = if right_left.field().color() == true {
                  // In the following diagrams, ||, //, and
                  // indicate the path to the removed node.
                  //
                  //      ||
                  //    pathp(r)
                  //  //        \
                  // (b)        (b)
                  //           /
                  //          (r)
                  //
                  cur.node.field().set_color(false);
                  cur.node.field().set_right(right.rotate_right());
                  cur.node.rotate_left()
                } else {
                  //      ||
                  //    pathp(r)
                  //  //        \
                  // (b)        (b)
                  //           /
                  //          (b)
                  //
                  cur.node.rotate_left()
                };
                // Balance restored, but rotation modified subtree
                // root.
                assert!(cur as *mut PathElem<T> > first_elem);
                match next.cmp {
                  None | Some(Less) => next.node.field().left = tnode,
                  _ => next.node.field().set_right(tnode),
                }
                return
              } else {
                let mut right = cur.node.field().right();
                let right_left = right.field().left;
                if right_left.field().color() == true {
                  //      ||
                  //    pathp(b)
                  //  //        \
                  // (b)        (b)
                  //           /
                  //          (r)
                  right_left.field().set_color(false);
                  cur.node.field().set_right(right.rotate_right());
                  let tnode = cur.node.rotate_left();
                  // Balance restored, but rotation modified
                  // subtree root, which may actually be the tree
                  // root.
                  if cur as *mut PathElem<T> == first_elem {
                    self.root = tnode;
                  } else {
                    match next.cmp {
                      None | Some(Less) => next.node.field().left = tnode,
                      _                 => next.node.field().set_right(tnode),
                    }
                  }
                  return
                } else {
                  //      ||
                  //    pathp(b)
                  //  //        \
                  // (b)        (b)
                  //           /
                  //          (b)
                  cur.node.field().set_color(true);
                  cur.node = cur.node.rotate_left();
                }
              }
            },
            Some(Greater)     => {
              cur.node.field().set_right(prev.node);
              let left = cur.node.field().left;
              if left.field().color() == true {
                let tnode;
                let left_right = left.field().right();
                let left_right_left = left_right.field().left;
                if left_right_left.field().color() == true {
                  //      ||
                  //    pathp(b)
                  //   /        \\
                  // (r)        (b)
                  //   \
                  //   (b)
                  //   /
                  // (r)
                  left_right_left.field().set_color(false);
                  let mut unode = cur.node.rotate_right();
                  unode.field().set_right(cur.node.rotate_right());
                  tnode = unode.rotate_left();
                } else {
                  //      ||
                  //    pathp(b)
                  //   /        \\
                  // (r)        (b)
                  //   \
                  //   (b)
                  //   /
                  // (b)
                  assert!(left_right != self.nil_ref());
                  left_right.field().set_color(true);
                  tnode = cur.node.rotate_right();
                  tnode.field().set_color(false);
                }
                // Balance restored, but rotation modified subtree
                // root, which may actually be the tree root.
                if cur as *mut PathElem<T> == first_elem {
                  // Set root.
                  self.root = tnode;
                } else {
                  match next.cmp {
                    None | Some(Less) => next.node.field().left = tnode,
                    _                 => next.node.field().set_right(tnode),
                  }
                }
                return;
              } else if cur.node.field().color() == true {
                let left_left = left.field().left;
                if left_left.field().color() == true {
                  //        ||
                  //      pathp(r)
                  //     /        \\
                  //   (b)        (b)
                  //   /
                  // (r)
                  cur.node.field().set_color(false);
                  left.field().set_color(true);
                  left_left.field().set_color(false);
                  let tnode = cur.node.rotate_right();
                  // Balance restored, but rotation modified
                  // subtree root.
                  assert!(cur as *mut PathElem<T> > first_elem);
                  match next.cmp {
                    None | Some(Less) => next.node.field().left = tnode,
                    _                 => next.node.field().set_right(tnode),
                  }
                  return;
                } else {
                  //        ||
                  //      pathp(r)
                  //     /        \\
                  //   (b)        (b)
                  //   /
                  // (b)
                  left.field().set_color(true);
                  cur.node.field().set_color(false);
                  // Balance restored.
                  return;
                }
              } else {
                let left_left = left.field().left;
                if left_left.field().color() == true {
                  //               ||
                  //             pathp(b)
                  //            /        \\
                  //          (b)        (b)
                  //          /
                  //        (r)
                  left_left.field().set_color(false);
                  let tnode = cur.node.rotate_right();
                  // Balance restored, but rotation modified
                  // subtree root, which may actually be the tree
                  // root.
                  if cur as *mut PathElem<T> == first_elem {
                    // Set root.
                    self.root = tnode;
                  } else {
                    match next.cmp {
                      None | Some(Less) => next.node.field().left = tnode,
                      _                 => next.node.field().set_right(tnode),
                    }
                  }
                  return;
                } else {
                  //               ||
                  //             pathp(b)
                  //            /        \\
                  //          (b)        (b)
                  //          /
                  //        (b)
                  left.field().set_color(true);
                }
              }
            }
          }
          prev = cur;
          cur = next;
        }
      }
    }
    // Set root
    self.root = path[0].node;
    assert_eq!(!self.root.field().color(), true);
  }


  fn iter_recur<F, A>(&mut self, node: *mut T, cb: &mut F) -> Option<A>
    where F: FnMut(&mut Self, *mut T) -> Option<A>
  {
    if node == self.nil_ref() {
      None
    } else {
      self.iter_recur(node.field().left, cb)
        .or_else(|:| (*cb)(self, node))
        .or_else(|:| self.iter_recur(node.field().right(), cb))
    }
  }

  fn iter_start<F, A>(&mut self, start: &mut T, node: *mut T, cb: &mut F) -> Option<A>
    where F: FnMut(&mut Self, *mut T) -> Option<A>
  {
    match (*start).partial_cmp(unsafe { &*node }) {
      None | Some(Less) => {
      self.iter_start(start, node.field().left, cb)
          .or_else(|:| (*cb)(self, node))
          .or_else(|:| self.iter_recur(node.field().right(), cb))
      },
      Some(Greater)     => self.iter_start(start, node.field().right(), cb),
      Some(Equal)       => {
        (*cb)(self, node)
          .or_else(|:| self.iter_recur(node.field().right(), cb))
      },
    }
  }

  pub fn iter<F, A>(&mut self, start: Option<&mut T>, cb: &mut F) -> Option<A>
    where F: FnMut(&mut Self, *mut T) -> Option<A>
  {
    match start {
      Some(start) => self.iter_start(start, self.root, cb),
      None        => self.iter_recur(self.root, cb),
    }
  }


  fn reverse_iter_recur<F, A>(&mut self, node: *mut T, cb: &mut F) -> Option<A>
    where F: FnMut(&mut Self, *mut T) -> Option<A>
  {
    if node == self.nil_ref() {
      None
    } else {
      self.reverse_iter_recur(node.field().right(), cb)
        .or_else(|:| (*cb)(self, node))
        .or_else(|:| self.reverse_iter_recur(node.field().left, cb))
    }
  }

  fn reverse_iter_start<F, A>(&mut self, start: &mut T, node: *mut T, cb: &mut F) -> Option<A>
    where F: FnMut(&mut Self, *mut T) -> Option<A>
  {
    match (*start).partial_cmp(unsafe { &*node }) {
      None | Some(Less) => {
        self.reverse_iter_start(start, node.field().right(), cb)
          .or_else(|:| (*cb)(self, node))
          .or_else(|:| self.reverse_iter_recur(node.field().left, cb))
      },
      Some(Greater)     => self.reverse_iter_start(start, node.field().left, cb),
      Some(Equal)       => {
        (*cb)(self, node)
          .or_else(|:| self.reverse_iter_recur(node.field().left, cb))
      },
    }
  }

  pub fn reverse_iter<F, A>(&mut self, start: Option<&mut T>, cb: &mut F) -> Option<A>
    where F: FnMut(&mut Self, *mut T) -> Option<A>
  {
    match start {
      Some(start) => self.reverse_iter_start(start, self.root, cb),
      None        => self.reverse_iter_recur(self.root, cb),
    }
  }
}

struct PathElem<T> {
  node: *mut T,
  cmp:  Option<Ordering>,
}
