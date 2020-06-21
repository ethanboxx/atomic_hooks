use crate::atom_state_access::{*, AtomStateAccess};
use crate::atom_store::{ Computed,Getter,AtomStore};
use std::cell::RefCell;

// use slotmap::{DenseSlotMap,DefaultKey, Key, SecondaryMap, SlotMap};

thread_local! {
    pub static ATOM_STORE: RefCell<AtomStore> = RefCell::new(AtomStore::new());
}

// 
//  Constructs a T atom state accessor. T is stored keyed to the provided String id.
//  The accessor always references this id therefore can you can set/update/ or get this T
//  from anywhere.
// 
//   The passed closure is only used for the first initialisation of state.
//   Subsequent evaluations of this function just returns the accessor.
//   Only one type per context can be stored in this way.
// 
//
// Typically this is created via the #[atom] attribute macro
//
pub fn atom<T: 'static , F: FnOnce() -> T, U,A>(current_id: &str, data_fn: F)  -> AtomStateAccess<T,U,IsAnAtomState> {
    
    // we do not need to re-initalize the atom if it already has been stored.
    if !atom_state_exists_for_id::<T>(current_id) {
        set_atom_state_with_id::<T>(data_fn(), current_id);
        ATOM_STORE.with(|store_refcell| {
            store_refcell
                .borrow_mut().add_atom(current_id);
        })
    }
    AtomStateAccess::new(current_id)
}

// 
//  Constructs a T computed state accessor. T is stored keyed to the provided String id.
//  The accessor always references this id. Typically computed values are auto
//  created based on changes to their dependencies which could be other computed values or an
//  atom state.
//
//   The passed closure is run whenever a dependency of the computed state has been updated.
// 
//
// Typically this is created via the #[computed] attribute macro
//
pub fn computed<T:Clone + 'static,U,A>(
    current_id: &str, 
    data_fn: fn(&str)->()) -> AtomStateAccess<T,NoUndo,IsAComputedState> {

    if !atom_state_exists_for_id::<T>(current_id) {
        ATOM_STORE.with(|store_refcell| {

            let key = store_refcell
                .borrow_mut()
                .primary_slotmap.insert(current_id.to_string());

            store_refcell.borrow_mut().id_to_key_map.insert(current_id.to_string(), key);
        });
        
    
        let computed = Computed{
            func: data_fn,
        };

        ((computed.func).clone())(current_id);
        
        ATOM_STORE.with(|store_refcell| {
            
            store_refcell
                .borrow_mut()
                .new_computed( current_id, computed);
        });
    }


    AtomStateAccess::<T,NoUndo,IsAComputedState>::new(current_id)
}




pub fn undo_atom_state<T: 'static + Clone, AllowUndo,IsAnAtomState>(current_id: &str){
    
    let mut undo_vec = remove_atom_state_with_id::<UndoVec<T>>(current_id).expect("untitlal undo vec to be present");
    
    if undo_vec.0.len() > 1 {
        let item =  undo_vec.0.pop().expect("type to exist");    
        update_atom_state_with_id(current_id,|t| *t = item);
        
    }
    set_atom_state_with_id(undo_vec, current_id) ;

}

pub fn atom_with_undo<T: 'static , F: FnOnce() -> T, U,A>(current_id: &str, data_fn: F)  -> AtomStateAccess<T,AllowUndo,IsAnAtomState> where T:Clone + 'static{
    
    if !atom_state_exists_for_id::<T>(current_id) {
        let item = data_fn();
        set_atom_state_with_id::<T>(item.clone(), current_id);
        set_atom_state_with_id(UndoVec::<T>(vec![item]), current_id);
        ATOM_STORE.with(|store_refcell| {
            store_refcell
                .borrow_mut().add_atom(current_id);
        })
    }
    AtomStateAccess::new(current_id)
}

pub fn link_state<T,U,A>(access : AtomStateAccess<T,U,A>) -> T where T:Clone + 'static{
    let getter =   illicit::Env::get::<RefCell<Getter>>().unwrap();
    getter.borrow_mut().atom_state_accessors.push(access.id.clone());

    ATOM_STORE.with(|store_refcell| {
        store_refcell
            .borrow_mut()
            .add_dependency(&access.id, &getter.borrow().computed_key);
    });

    clone_atom_state_with_id::<T>(&access.id).unwrap()
}



pub fn set_atom_state_with_id_with_undo<T: 'static>(data: T, current_id: &str) where T:Clone {
    let item = clone_atom_state_with_id::<T>(current_id).expect("inital state needs to be present");
    let mut  undo_vec = remove_atom_state_with_id::<UndoVec<T>>(current_id).expect("untitlal undo vec to be present");
    undo_vec.0.push(item);
    set_atom_state_with_id(undo_vec, current_id) ;
    set_atom_state_with_id(data, current_id);
}


/// Sets the state of type T keyed to the given TopoId
pub fn set_atom_state_with_id<T: 'static>(data: T, current_id: &str) {
    ATOM_STORE.with(|store_refcell| {
        store_refcell
            .borrow_mut()
            .set_state_with_id::<T>(data, current_id)
    })
}


pub fn atom_state_exists_for_id<T: 'static>(id: &str) -> bool {
    ATOM_STORE.with(|store_refcell| store_refcell.borrow().state_exists_with_id::<T>(id))
}


/// Clones the state of type T keyed to the given TopoId
pub fn clone_atom_state_with_id<T: 'static + Clone>(id: &str) -> Option<T> {
    ATOM_STORE.with(|store_refcell| {
        store_refcell
            .borrow_mut()
            .get_state_with_id::<T>(id)
            .cloned()
    })
}

pub fn remove_atom_state_with_id<T: 'static>(id: &str) -> Option<T> {
    ATOM_STORE.with(|store_refcell| {
        store_refcell
            .borrow_mut()
            .remove_state_with_id::<T>(id)
    })
}

// Provides mutable access to the stored state type T.
//
// Example:
//
// ```
// update_state_with_topo_id::<Vec<String>>( topo::Id::current(), |v|
//     v.push("foo".to_string()
// )

//

#[derive(Clone)]
pub struct UndoVec<T>(pub Vec<T>);

pub fn update_atom_state_with_id_with_undo<T: 'static, F: FnOnce(&mut T) -> ()>(id: &str, func: F) where T:Clone{

    let mut item = remove_atom_state_with_id::<T>(id)
        .expect("You are trying to update a type state that doesnt exist in this context!");

    
    let mut undo_vec = remove_atom_state_with_id::<UndoVec<T>>(id)
        .expect("You are trying to update a type state that doesnt exist in this context!");
    undo_vec.0.push(item.clone());

    set_atom_state_with_id(undo_vec, id);
    

    func(&mut item);
    set_atom_state_with_id(item, id);

    //we need to get the associated data with this key
    
    
    execute_computed_nodes(id);
}

fn execute_computed_nodes(id: &str) {
    let ids_computeds = ATOM_STORE.with(|refcell_store|{
        let mut borrow = refcell_store.borrow_mut();
        borrow.clone_dep_funcs_for_id(id)
    });

    for (key,computed) in ids_computeds {
        (computed.func)(&key);
        execute_computed_nodes(&key);
    }

}



pub fn update_atom_state_with_id<T: 'static, F: FnOnce(&mut T) -> ()>(id: &str, func: F) {
    let mut item = remove_atom_state_with_id::<T>(id)
        .expect("You are trying to update a type state that doesnt exist in this context!");

    func(&mut item);

    set_atom_state_with_id(item, id);

    //we need to get the associated data with this key
    
    execute_computed_nodes(id);

}

pub fn read_atom_state_with_id<T: 'static, F: FnOnce(&T) -> R, R>(id: &str, func: F) -> R {
    let item = remove_atom_state_with_id::<T>(id)
        .expect("You are trying to read a type state that doesnt exist in this context!");
    let read = func(&item);
    set_atom_state_with_id(item, id);
    read
}