import { Flex } from "@radix-ui/themes";
import { Chat } from "./features/Chat";
import { useEventBusForHost } from "./hooks/useEventBusForHost";
import { HistorySideBar } from "./features/HistorySideBar";
import { PageWrapper } from "./components/PageWrapper";
import { Theme } from "./components/Theme";

function App() {
  useEventBusForHost();
  return (
    <Theme>
      <Flex>
        <HistorySideBar />
        <PageWrapper>
          <Chat />
        </PageWrapper>
      </Flex>
    </Theme>
  );
}

export default App;
