// ============================================================
//  pallet-todo  – A simple on-chain Todo list pallet
// ============================================================
//
//  Storage layout
//  ──────────────
//  Todos       : Map< (AccountId, TodoId) → TodoItem >
//  NextTodoId  : Map< AccountId → TodoId >             (per-user auto-increment)
//
//  Extrinsics
//  ──────────
//  create_todo(title, description)   – add a new item
//  complete_todo(todo_id)            – mark an item as done
//  remove_todo(todo_id)              – delete an item
//  update_todo(todo_id, new_title, new_description) – edit an item

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::{
        pallet_prelude::*,
        traits::Get,
    };
    use frame_system::pallet_prelude::*;

    // ──────────────────────────────────────────────────────────
    //  Type aliases
    // ──────────────────────────────────────────────────────────
    pub type TodoId = u64;

    // ──────────────────────────────────────────────────────────
    //  On-chain data structure
    // ──────────────────────────────────────────────────────────
    /// Represents a single todo item stored on-chain.
    #[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    #[scale_info(skip_type_params(T))]
    pub struct TodoItem<T: Config> {
        /// Sequential ID, unique per owner.
        pub id: TodoId,
        /// Short title of the task (bounded string).
        pub title: BoundedVec<u8, T::MaxTitleLength>,
        /// Optional longer description.
        pub description: BoundedVec<u8, T::MaxDescriptionLength>,
        /// Whether the task has been completed.
        pub completed: bool,
        /// Block number at which this item was created.
        pub created_at: BlockNumberFor<T>,
    }

    // ──────────────────────────────────────────────────────────
    //  Pallet definition
    // ──────────────────────────────────────────────────────────
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    // ──────────────────────────────────────────────────────────
    //  Config trait
    // ──────────────────────────────────────────────────────────
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Maximum length of a todo title (bytes).
        #[pallet::constant]
        type MaxTitleLength: Get<u32>;

        /// Maximum length of a todo description (bytes).
        #[pallet::constant]
        type MaxDescriptionLength: Get<u32>;

        /// Maximum number of todos a single account may have at any time.
        #[pallet::constant]
        type MaxTodosPerAccount: Get<u32>;
    }

    // ──────────────────────────────────────────────────────────
    //  Storage
    // ──────────────────────────────────────────────────────────

    /// Stores all todo items: (owner, todo_id) → TodoItem
    #[pallet::storage]
    #[pallet::getter(fn todos)]
    pub type Todos<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,   // owner
        Blake2_128Concat,
        TodoId,         // item id (per-owner)
        TodoItem<T>,
        OptionQuery,
    >;

    /// Tracks the next available todo ID per account (monotonically increasing).
    #[pallet::storage]
    #[pallet::getter(fn next_todo_id)]
    pub type NextTodoId<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, TodoId, ValueQuery>;

    /// Tracks the current number of todos per account (for cap enforcement).
    #[pallet::storage]
    #[pallet::getter(fn todo_count)]
    pub type TodoCount<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

    // ──────────────────────────────────────────────────────────
    //  Events
    // ──────────────────────────────────────────────────────────
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new todo was created.  [owner, todo_id, title]
        TodoCreated { owner: T::AccountId, todo_id: TodoId },
        /// A todo was marked as completed.  [owner, todo_id]
        TodoCompleted { owner: T::AccountId, todo_id: TodoId },
        /// A todo was removed.  [owner, todo_id]
        TodoRemoved { owner: T::AccountId, todo_id: TodoId },
        /// A todo was updated. [owner, todo_id]
        TodoUpdated { owner: T::AccountId, todo_id: TodoId },
    }

    // ──────────────────────────────────────────────────────────
    //  Errors
    // ──────────────────────────────────────────────────────────
    #[pallet::error]
    pub enum Error<T> {
        /// The todo with the given ID does not exist (or does not belong to the caller).
        TodoNotFound,
        /// The title exceeds `MaxTitleLength`.
        TitleTooLong,
        /// The description exceeds `MaxDescriptionLength`.
        DescriptionTooLong,
        /// The todo is already marked as completed.
        AlreadyCompleted,
        /// The caller has reached the per-account todo limit.
        TooManyTodos,
        /// TodoId would overflow u64 (practically unreachable).
        TodoIdOverflow,
    }

    // ──────────────────────────────────────────────────────────
    //  Extrinsics (calls)
    // ──────────────────────────────────────────────────────────
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Create a new todo item.
        ///
        /// - `title`       – short name for the task (≤ MaxTitleLength bytes)
        /// - `description` – optional longer description (≤ MaxDescriptionLength bytes)
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10_000, 0))]
        pub fn create_todo(
            origin: OriginFor<T>,
            title: BoundedVec<u8, T::MaxTitleLength>,
            description: BoundedVec<u8, T::MaxDescriptionLength>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Enforce per-account cap
            let count = TodoCount::<T>::get(&who);
            ensure!(count < T::MaxTodosPerAccount::get(), Error::<T>::TooManyTodos);

            // Allocate an ID
            let todo_id = NextTodoId::<T>::get(&who);
            let next_id = todo_id.checked_add(1).ok_or(Error::<T>::TodoIdOverflow)?;

            let item = TodoItem::<T> {
                id: todo_id,
                title,
                description,
                completed: false,
                created_at: <frame_system::Pallet<T>>::block_number(),
            };

            Todos::<T>::insert(&who, todo_id, item);
            NextTodoId::<T>::insert(&who, next_id);
            TodoCount::<T>::insert(&who, count + 1);

            Self::deposit_event(Event::TodoCreated { owner: who, todo_id });
            Ok(())
        }

        /// Mark an existing todo as completed.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(10_000, 0))]
        pub fn complete_todo(origin: OriginFor<T>, todo_id: TodoId) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Todos::<T>::try_mutate(&who, todo_id, |maybe_item| -> DispatchResult {
                let item = maybe_item.as_mut().ok_or(Error::<T>::TodoNotFound)?;
                ensure!(!item.completed, Error::<T>::AlreadyCompleted);
                item.completed = true;
                Ok(())
            })?;

            Self::deposit_event(Event::TodoCompleted { owner: who, todo_id });
            Ok(())
        }

        /// Remove a todo item permanently.
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(10_000, 0))]
        pub fn remove_todo(origin: OriginFor<T>, todo_id: TodoId) -> DispatchResult {
            let who = ensure_signed(origin)?;

            ensure!(
                Todos::<T>::contains_key(&who, todo_id),
                Error::<T>::TodoNotFound
            );

            Todos::<T>::remove(&who, todo_id);
            TodoCount::<T>::mutate(&who, |c| *c = c.saturating_sub(1));

            Self::deposit_event(Event::TodoRemoved { owner: who, todo_id });
            Ok(())
        }

        /// Update the title and/or description of an existing (non-completed) todo.
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(10_000, 0))]
        pub fn update_todo(
            origin: OriginFor<T>,
            todo_id: TodoId,
            new_title: BoundedVec<u8, T::MaxTitleLength>,
            new_description: BoundedVec<u8, T::MaxDescriptionLength>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Todos::<T>::try_mutate(&who, todo_id, |maybe_item| -> DispatchResult {
                let item = maybe_item.as_mut().ok_or(Error::<T>::TodoNotFound)?;
                item.title = new_title;
                item.description = new_description;
                Ok(())
            })?;

            Self::deposit_event(Event::TodoUpdated { owner: who, todo_id });
            Ok(())
        }
    }
}
