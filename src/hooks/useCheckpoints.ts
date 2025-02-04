import { useCallback, useMemo } from "react";
import { useAppSelector } from "./useAppSelector";
import {
  selectCheckpointsMessageIndex,
  selectIsCheckpointsPopupIsVisible,
  selectIsUndoingCheckpoints,
  selectLatestCheckpointResult,
  selectShouldNewChatBeStarted,
  setIsCheckpointsPopupIsVisible,
  setIsUndoingCheckpoints,
  setLatestCheckpointResult,
  setShouldNewChatBeStarted,
} from "../features/Checkpoints/checkpointsSlice";
import { useAppDispatch } from "./useAppDispatch";
import { useRestoreCheckpoints } from "./useRestoreCheckpoints";
import { Checkpoint, FileChanged } from "../features/Checkpoints/types";
import {
  backUpMessages,
  newChatAction,
  selectChatId,
  selectMessages,
} from "../features/Chat";
import { isUserMessage } from "../events";
import { deleteChatById } from "../features/History/historySlice";

export const useCheckpoints = () => {
  const dispatch = useAppDispatch();
  const messages = useAppSelector(selectMessages);
  const chatId = useAppSelector(selectChatId);
  const { restoreChangesFromCheckpoints, isLoading } = useRestoreCheckpoints();
  const isCheckpointsPopupVisible = useAppSelector(
    selectIsCheckpointsPopupIsVisible,
  );
  const isUndoingCheckpoints = useAppSelector(selectIsUndoingCheckpoints);

  const latestRestoredCheckpointsResult = useAppSelector(
    selectLatestCheckpointResult,
  );

  const { reverted_changes, reverted_to } = latestRestoredCheckpointsResult;

  const shouldNewChatBeStarted = useAppSelector(selectShouldNewChatBeStarted);
  const maybeMessageIndex = useAppSelector(selectCheckpointsMessageIndex);

  const allChangedFiles = reverted_changes.reduce<
    (FileChanged & { workspace_folder: string })[]
  >((acc, change) => {
    const filesWithWorkspace = change.files_changed.map((file) => ({
      ...file,
      workspace_folder: change.workspace_folder,
    }));
    return [...acc, ...filesWithWorkspace];
  }, []);

  const wereFilesChanged = useMemo(() => {
    return allChangedFiles.length > 0;
  }, [allChangedFiles]);

  const shouldCheckpointsPopupBeShown = useMemo(() => {
    return isCheckpointsPopupVisible && !isUndoingCheckpoints;
  }, [isCheckpointsPopupVisible, isUndoingCheckpoints]);

  const handleUndo = useCallback(async () => {
    await restoreChangesFromCheckpoints(
      latestRestoredCheckpointsResult.checkpoints_for_undo,
    );
    dispatch(setIsUndoingCheckpoints(true));
  }, [
    dispatch,
    restoreChangesFromCheckpoints,
    latestRestoredCheckpointsResult.checkpoints_for_undo,
  ]);

  const handleRestore = useCallback(
    async (checkpoints: Checkpoint[] | null, messageIndex: number) => {
      if (!checkpoints) return;
      const amountOfUserMessages = messages.filter(isUserMessage);
      const firstUserMessage = amountOfUserMessages[0];

      const restoredChanges =
        await restoreChangesFromCheckpoints(checkpoints).unwrap();

      const actions = [
        dispatch(setIsUndoingCheckpoints(false)),
        setLatestCheckpointResult({ ...restoredChanges, messageIndex }),
        setIsCheckpointsPopupIsVisible(true),
        setShouldNewChatBeStarted(
          messageIndex === messages.indexOf(firstUserMessage),
        ),
      ];
      actions.forEach((action) => dispatch(action));
    },
    [dispatch, restoreChangesFromCheckpoints, messages],
  );

  const handleFix = useCallback(() => {
    dispatch(setIsCheckpointsPopupIsVisible(false));
    if (shouldNewChatBeStarted || !maybeMessageIndex) {
      const actions = [newChatAction(), deleteChatById(chatId)];
      actions.forEach((action) => dispatch(action));
    } else {
      const usefulMessages = messages.slice(0, maybeMessageIndex);
      dispatch(
        backUpMessages({
          id: chatId,
          messages: usefulMessages,
        }),
      );
    }
  }, [dispatch, shouldNewChatBeStarted, maybeMessageIndex, chatId, messages]);

  return {
    shouldCheckpointsPopupBeShown,
    handleUndo,
    handleRestore,
    handleFix,
    isLoading,
    reverted_changes,
    reverted_to,
    wereFilesChanged,
    allChangedFiles,
  };
};
