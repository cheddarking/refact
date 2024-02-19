import React, { useEffect, useState } from "react";
import { Box, Flex, Button, Heading, Responsive } from "@radix-ui/themes";
import { RefactTableData } from "../../services/refact";
import { Table } from "../Table/Table";
import { Chart } from "../Chart/Chart";
import { Spinner } from "../Spinner";
import { ArrowLeftIcon } from "@radix-ui/react-icons";
import { useConfig } from "../../contexts/config-context";
import { ScrollArea } from "../ScrollArea";

const table: { data: string } = {
  data: '{"refact_impact_dates":{"data":{"daily":{"2023-12-15":{"completions":14,"human":52,"langs":[".rs",".py"],"refact":203,"refact_impact":0.7960784435272217,"total":255},"2023-12-18":{"completions":16,"human":83,"langs":[".py"],"refact":245,"refact_impact":0.7469512224197388,"total":328},"2023-12-19":{"completions":6,"human":4,"langs":[".cpp"],"refact":103,"refact_impact":0.9626168012619019,"total":107},"2023-12-20":{"completions":46,"human":857,"langs":[".py"],"refact":693,"refact_impact":0.4470967650413513,"total":1550},"2023-12-21":{"completions":92,"human":1157,"langs":[".py"],"refact":3103,"refact_impact":0.7284037470817566,"total":4260},"2023-12-22":{"completions":59,"human":-38,"langs":[".py"],"refact":2005,"refact_impact":1.0193188190460205,"total":1967},"2023-12-27":{"completions":13,"human":28,"langs":[".py"],"refact":409,"refact_impact":0.9359267950057983,"total":437},"2023-12-29":{"completions":2,"human":2,"langs":[".py"],"refact":71,"refact_impact":0.9726027250289917,"total":73},"2024-01-04":{"completions":12,"human":1772,"langs":[".rs"],"refact":303,"refact_impact":0.14602409303188324,"total":2075},"2024-01-09":{"completions":4,"human":33,"langs":[".py"],"refact":166,"refact_impact":0.8341708779335022,"total":199},"2024-01-24":{"completions":10,"human":808,"langs":[".rs"],"refact":410,"refact_impact":0.3366174101829529,"total":1218},"2024-01-25":{"completions":76,"human":7993,"langs":[".rs"],"refact":2772,"refact_impact":0.25750115513801575,"total":10765},"2024-01-26":{"completions":21,"human":1931,"langs":[".rs"],"refact":557,"refact_impact":0.22387459874153137,"total":2488},"2024-01-29":{"completions":21,"human":2574,"langs":[".rs"],"refact":655,"refact_impact":0.20284917950630188,"total":3229},"2024-01-30":{"completions":29,"human":1849,"langs":[".rs"],"refact":1310,"refact_impact":0.41468819975852966,"total":3159},"2024-01-31":{"completions":31,"human":3452,"langs":[".rs",".txt"],"refact":1114,"refact_impact":0.24397721886634827,"total":4566},"2024-02-01":{"completions":57,"human":8806,"langs":[".rs",".txt"],"refact":2465,"refact_impact":0.21870286762714386,"total":11271},"2024-02-02":{"completions":11,"human":5869,"langs":[".rs",".txt",".py"],"refact":307,"refact_impact":0.04970854893326759,"total":6176},"2024-02-05":{"completions":5,"human":1976,"langs":[".rs",".txt"],"refact":233,"refact_impact":0.10547759383916855,"total":2209}},"weekly":{"2023-12-15":{"completions":14,"human":52,"langs":[".py",".rs"],"refact":203,"refact_impact":0.7960784435272217,"total":255},"2023-12-22":{"completions":219,"human":2063,"langs":[".py",".cpp"],"refact":6149,"refact_impact":0.7487822771072388,"total":8212},"2023-12-27":{"completions":15,"human":30,"langs":[".py"],"refact":480,"refact_impact":0.9411764740943909,"total":510},"2024-01-04":{"completions":12,"human":1772,"langs":[".rs"],"refact":303,"refact_impact":0.14602409303188324,"total":2075},"2024-01-09":{"completions":4,"human":33,"langs":[".py"],"refact":166,"refact_impact":0.8341708779335022,"total":199},"2024-01-24":{"completions":107,"human":10732,"langs":[".rs"],"refact":3739,"refact_impact":0.2583788335323334,"total":14471},"2024-02-02":{"completions":149,"human":22550,"langs":[".rs",".py",".txt"],"refact":5851,"refact_impact":0.20601387321949005,"total":28401},"2024-02-05":{"completions":5,"human":1976,"langs":[".rs",".txt"],"refact":233,"refact_impact":0.10547759383916855,"total":2209}}}},"table_refact_impact":{"columns":["Language","Refact","Human","Total (characters)","Refact Impact","Completions"],"data":[{"completions":276,"human":31996,"lang":".rs","refact":10092,"refact_impact":0.23978331685066223,"total":42088},{"completions":243,"human":7110,"lang":".py","refact":6929,"refact_impact":0.49355366826057434,"total":14039},{"completions":6,"human":4,"lang":".cpp","refact":103,"refact_impact":0.9626168012619019,"total":107},{"completions":0,"human":98,"lang":".txt","refact":0,"refact_impact":0.0,"total":98}],"title":"Refact\'s impact by language"}}',
};

export const Statistic: React.FC<{
  onCloseStatistic?: () => void;
  backFromChat: () => void;
}> = ({ onCloseStatistic, backFromChat }) => {
  const [isLoaded, setIsLoaded] = useState<boolean>(false);
  const [refactTable, setRefactTable] = useState<RefactTableData | null>(null);
  const { host, tabbed } = useConfig();

  const LeftRightPadding: Responsive<
    "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"
  > =
    host === "web"
      ? { initial: "2", xl: "9" }
      : {
          initial: "2",
          xs: "2",
          sm: "4",
          md: "8",
          lg: "8",
          xl: "9",
        };

  const TopBottomPadding: Responsive<
    "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"
  > = {
    initial: "5",
  };

  useEffect(() => {
    if (table.data) {
      setRefactTable(JSON.parse(table.data) as RefactTableData);
      setIsLoaded(true);
    }
  }, []);

  return (
    <Flex
      direction="column"
      justify="between"
      grow="1"
      pl={LeftRightPadding}
      pt={TopBottomPadding}
      pb={TopBottomPadding}
      style={{
        height: "100dvh",
      }}
    >
      {host === "vscode" && !tabbed ? (
        <Flex gap="2" pb="3">
          <Button variant="surface" onClick={backFromChat}>
            <ArrowLeftIcon width="16" height="16" />
            Back
          </Button>
        </Flex>
      ) : (
        <Button mr="auto" variant="outline" onClick={onCloseStatistic} mb="4">
          Back
        </Button>
      )}
      <ScrollArea scrollbars="vertical">
        <Flex
          direction="column"
          justify="between"
          grow="1"
          mr={LeftRightPadding}
          style={{
            width: "inherit",
          }}
        >
          {isLoaded ? (
            <Box
              style={{
                width: "inherit",
              }}
            >
              <Flex
                direction="column"
                style={{
                  width: "inherit",
                }}
              >
                <Heading as="h3" align="center" mb="1">
                  Statistics
                </Heading>
                {refactTable !== null && (
                  <Flex align="center" justify="center" direction="column">
                    <Table
                      refactImpactTable={refactTable.table_refact_impact.data}
                    />
                    <Chart
                      refactImpactDatesWeekly={
                        refactTable.refact_impact_dates.data.weekly
                      }
                    />
                  </Flex>
                )}
              </Flex>
            </Box>
          ) : (
            <Spinner />
          )}
        </Flex>
      </ScrollArea>
    </Flex>
  );
};
