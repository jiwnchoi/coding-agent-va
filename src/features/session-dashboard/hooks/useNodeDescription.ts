import { Channel, invoke } from "@tauri-apps/api/core";
import { useCallback, useRef, useState } from "react";

import type {
  AgentSessionNodeDescriptionRequest,
  AgentSessionNodeDescriptionResponse,
  AgentSessionNodeDescriptionStreamEvent,
} from "@/shared/lib/generated/bindings";

export function useNodeDescription() {
  const requestVersion = useRef(0);
  const animationFrame = useRef<number | null>(null);
  const pendingText = useRef("");
  const isCachedResponse = useRef(false);
  const isCommandFinished = useRef(false);
  const [description, setDescription] = useState("");
  const [errorMessage, setErrorMessage] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [providerLabel, setProviderLabel] = useState("");

  const reset = useCallback(() => {
    requestVersion.current += 1;
    if (animationFrame.current !== null) cancelAnimationFrame(animationFrame.current);
    animationFrame.current = null;
    pendingText.current = "";
    isCachedResponse.current = false;
    isCommandFinished.current = false;
    setDescription("");
    setErrorMessage("");
    setIsLoading(false);
    setProviderLabel("");
  }, []);

  const renderPendingText = useCallback(function renderPendingText(version: number) {
    if (requestVersion.current !== version) return;
    const nextText = pendingText.current.slice(0, 48);
    pendingText.current = pendingText.current.slice(nextText.length);
    if (nextText) setDescription((current) => current + nextText);
    if (pendingText.current) {
      animationFrame.current = requestAnimationFrame(() => renderPendingText(version));
    } else {
      animationFrame.current = null;
      if (isCommandFinished.current) setIsLoading(false);
    }
  }, []);

  const appendDescription = useCallback(
    (text: string, version: number) => {
      if (isCachedResponse.current) {
        setDescription((current) => current + text);
        return;
      }
      pendingText.current += text;
      if (animationFrame.current === null) {
        animationFrame.current = requestAnimationFrame(() => renderPendingText(version));
      }
    },
    [renderPendingText]
  );

  const describe = useCallback(
    async (request: AgentSessionNodeDescriptionRequest) => {
      const currentVersion = requestVersion.current + 1;
      requestVersion.current = currentVersion;
      if (animationFrame.current !== null) cancelAnimationFrame(animationFrame.current);
      animationFrame.current = null;
      pendingText.current = "";
      isCachedResponse.current = false;
      isCommandFinished.current = false;
      setDescription("");
      setErrorMessage("");
      setIsLoading(true);
      setProviderLabel("");

      try {
        const onEvent = new Channel<AgentSessionNodeDescriptionStreamEvent>();
        onEvent.onmessage = (event) => {
          if (requestVersion.current !== currentVersion) return;
          if (event.type === "started") {
            isCachedResponse.current = event.cached;
            setProviderLabel(event.providerLabel);
          } else {
            appendDescription(event.text, currentVersion);
          }
        };
        const response = await invoke<AgentSessionNodeDescriptionResponse>(
          "describe_agent_session_node",
          { onEvent, request }
        );
        if (requestVersion.current !== currentVersion) {
          return;
        }
        setProviderLabel(response.providerLabel);
        isCommandFinished.current = true;
        if (!pendingText.current && animationFrame.current === null) setIsLoading(false);
      } catch (error) {
        if (requestVersion.current === currentVersion) {
          pendingText.current = "";
          if (animationFrame.current !== null) cancelAnimationFrame(animationFrame.current);
          animationFrame.current = null;
          setErrorMessage(error instanceof Error ? error.message : String(error));
          setIsLoading(false);
        }
      }
    },
    [appendDescription]
  );

  return { describe, description, errorMessage, isLoading, providerLabel, reset };
}
