import { type RefObject, useLayoutEffect, useState } from "react";

type ElementSize = { width: number; height: number };

const EMPTY_SIZE: ElementSize = { width: 0, height: 0 };

export function useElementSize(ref: RefObject<HTMLElement | null>) {
  const [size, setSize] = useState(EMPTY_SIZE);

  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;

    const updateSize = () => {
      const nextSize = { width: element.clientWidth, height: element.clientHeight };
      setSize((currentSize) =>
        currentSize.width === nextSize.width && currentSize.height === nextSize.height
          ? currentSize
          : nextSize
      );
    };
    const resizeObserver = new ResizeObserver(updateSize);
    updateSize();
    resizeObserver.observe(element);

    return () => resizeObserver.disconnect();
  }, [ref]);

  return size;
}
