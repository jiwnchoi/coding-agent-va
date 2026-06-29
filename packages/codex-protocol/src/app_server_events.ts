export type SessionStatus = "notLoaded" | "idle" | "active" | "systemError";

export type Session = {
  threadId: string;
  sessionId?: string;
  cwd?: string;
  status: SessionStatus;
  activeTurnId?: string;
};

export type PlanStep = {
  id: string;
  turnId: string;
  text: string;
  status: "pending" | "inProgress" | "completed";
  relatedNodeIds: string[];
};

export type CodexAppServerEvent =
  | { type: "thread/started"; session: Session; timestamp: number }
  | { type: "thread/status/changed"; threadId: string; status: SessionStatus; timestamp: number }
  | { type: "turn/started"; turnId: string; threadId: string; timestamp: number }
  | { type: "turn/completed"; turnId: string; threadId: string; timestamp: number }
  | {
      type: "turn/plan/updated";
      turnId: string;
      threadId: string;
      steps: PlanStep[];
      timestamp: number;
    }
  | {
      type: "turn/diff/updated";
      turnId: string;
      threadId: string;
      files: string[];
      timestamp: number;
    };
