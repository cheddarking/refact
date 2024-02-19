import React, { useEffect, useImperativeHandle, useRef } from "react";
import { TextArea as RadixTextArea } from "@radix-ui/themes";
import classNames from "classnames";
import styles from "./TextArea.module.css";

export type TextAreaProps = React.ComponentProps<typeof RadixTextArea> & {
  className?: string;
  value?: string;
  onTextAreaHeightChange?: (scrollHeight: number) => void;
};

export const TextArea = React.forwardRef<HTMLTextAreaElement, TextAreaProps>(
  ({ onTextAreaHeightChange, ...props }, ref) => {
    const innerRef = useRef<HTMLTextAreaElement>(null);
    // eslint-disable-next-line @typescript-eslint/no-non-null-assertion
    useImperativeHandle(ref, () => innerRef.current!, []);

    useEffect(() => {
      if (innerRef.current) {
        innerRef.current.style.height = "1px";
        innerRef.current.style.height =
          2 + innerRef.current.scrollHeight + "px";
        onTextAreaHeightChange &&
          onTextAreaHeightChange(innerRef.current.scrollHeight);
      }
    }, [innerRef.current?.value, onTextAreaHeightChange]);

    return (
      <RadixTextArea
        {...props}
        className={classNames(styles.textarea, props.className)}
        ref={innerRef}
      />
    );
  },
);

TextArea.displayName = "TextArea";
