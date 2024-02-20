import { useReducer } from "react";

enum ACTION_TYPES {
  SET_STATE = "SET_STATE",
  UNDO = "UNDO",
  REDO = "REDO",
}

interface InternalState<T> {
  past: T[];
  present: T;
  future: T[];
}

interface Action {
  type: ACTION_TYPES;
}

interface SetStateAction<T> extends Action {
  type: ACTION_TYPES.SET_STATE;
  payload: T;
}

interface UndoAction extends Action {
  type: ACTION_TYPES.UNDO;
}

interface RedoAction extends Action {
  type: ACTION_TYPES.REDO;
}

type Actions<T> = SetStateAction<T> | UndoAction | RedoAction;

const reducerWithUndoRedo = <T>(
  state: InternalState<T>,
  action: Actions<T>,
) => {
  const { past, present, future } = state;

  switch (action.type) {
    case ACTION_TYPES.SET_STATE: {
      return {
        past: [...past, present],
        present: action.payload,
        future: [],
      };
    }
    case ACTION_TYPES.UNDO: {
      return {
        past: past.slice(0, past.length - 1),
        present: past[past.length - 1],
        future: [present, ...future],
      };
    }
    case ACTION_TYPES.REDO: {
      return {
        past: [...past, present],
        present: future[0],
        future: future.slice(1),
      };
    }
    default: {
      return state;
    }
  }
};

const createInitialState = <T>(initialState: T): InternalState<T> => {
  return {
    past: [],
    present: initialState,
    future: [],
  };
};

export const useUndoRedo = <T>(initialState: T) => {
  const [state, dispatch] = useReducer(
    reducerWithUndoRedo<T>,
    createInitialState(initialState),
  );
  const { past, present, future } = state;

  const setState = (newState: T) =>
    dispatch({ type: ACTION_TYPES.SET_STATE, payload: newState });
  const undo = () => dispatch({ type: ACTION_TYPES.UNDO });
  const redo = () => dispatch({ type: ACTION_TYPES.REDO });
  const isUndoPossible = past.length > 0;
  const isRedoPossible = future.length > 0;

  return {
    state: present,
    setState,
    undo,
    redo,
    pastStates: past,
    futureStates: future,
    isUndoPossible,
    isRedoPossible,
  };
};
