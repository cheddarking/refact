import { useEffect } from "react";
import { sendChat } from "../services/refact";
import { ChatState } from "./useEventBusForChat";
import { useChatHistory } from "./useChatHistory";

export function useEventBusForHost() {
  const { history, saveChat } = useChatHistory();

  useEffect(() => {
    const controller = new AbortController();
    const listener = (event: MessageEvent) => {
      if (event.source !== window) {
        return;
      }
      // console.log("host");
      // console.log(event.data)
      // TODO: validate the events
      // eslint-disable-next-line @typescript-eslint/no-unsafe-member-access
      if (!event.data.type) {
        return;
      }
      // eslint-disable-next-line @typescript-eslint/no-unsafe-member-access
      switch (event.data.type) {
        case "chat_question": {
          // eslint-disable-next-line @typescript-eslint/no-unsafe-member-access
          const payload = event.data.payload as unknown as ChatState;
          saveChat({
            id: payload.id,
            title: payload.title ?? "",
            messages: payload.messages,
            model: payload.model || "gpt-3.5-turbo",
          });
          // eslint-disable-next-line @typescript-eslint/no-unsafe-member-access
          handleSend(event.data.payload as ChatState, controller);
          return;
        }
        case "save_chat_to_history": {
          // eslint-disable-next-line @typescript-eslint/no-unsafe-member-access
          const chat = event.data.payload as ChatState;
          saveChat(chat);
          return;
        }
      }
    };

    window.addEventListener("message", listener);

    return () => {
      controller.abort();
      window.removeEventListener("message", listener);
    };
  }, [saveChat]);

  return { history };
}

function handleSend(chat: ChatState, controller: AbortController) {
  sendChat(chat.messages, "gpt-3.5-turbo", controller)
    .then((response) => {
      const decoder = new TextDecoder();
      const reader = response.body?.getReader();
      if (!reader) return;
      void reader.read().then(function pump({ done, value }): Promise<void> {
        if (done) {
          // Do something with last chunk of data then exit reader
          return Promise.resolve();
        }

        const streamAsString = decoder.decode(value);

        const deltas = streamAsString
          .split("\n\n")
          .filter((str) => str.length > 0);
        if (deltas.length === 0) return Promise.resolve();

        for (const delta of deltas) {
          if (!delta.startsWith("data: ")) {
            console.log("Unexpected data in streaming buf: " + delta);
            continue;
          }

          const maybeJsonString = delta.substring(6);
          if (maybeJsonString === "[DONE]") {
            window.postMessage({ type: "chat_done_streaming" }, "*");
            return Promise.resolve(); // handle finish
          }

          if (maybeJsonString === "[ERROR]") {
            console.log("Streaming error");
            const errorJson = JSON.parse(maybeJsonString) as Record<
              string,
              unknown
            >;
            console.log(errorJson);
            window.postMessage({ type: "chat_error", payload: errorJson }, "*");
            return Promise.reject(errorJson.detail || "streaming error"); // handle error
          }
          // figure out how to safely parseJson

          const json = JSON.parse(maybeJsonString) as Record<string, unknown>;

          // console.log(json);
          window.postMessage(
            {
              type: "chat_response",
              payload: {
                id: chat.id,
                ...json,
              },
            },
            "*",
          );
        }

        return reader.read().then(pump);
      });
    })
    .catch(console.error);
}
