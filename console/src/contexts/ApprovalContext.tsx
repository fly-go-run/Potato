import React, { createContext, useContext, useState, ReactNode } from "react";
import { type PendingApproval } from "../api/modules/console";

interface ApprovalContextValue {
  approvals: PendingApproval[];
  setApprovals: React.Dispatch<React.SetStateAction<PendingApproval[]>>;
}

const ApprovalContext = createContext<ApprovalContextValue | undefined>(
  undefined,
);

export function ApprovalProvider({ children }: { children: ReactNode }) {
  const [approvals, setApprovals] = useState<PendingApproval[]>([]);

  return (
    <ApprovalContext.Provider value={{ approvals, setApprovals }}>
      {children}
    </ApprovalContext.Provider>
  );
}

// eslint-disable-next-line react-refresh/only-export-components -- This export is intentionally colocated as part of the component public API.
export function useApprovalContext() {
  const context = useContext(ApprovalContext);
  if (!context) {
    throw new Error("useApprovalContext must be used within ApprovalProvider");
  }
  return context;
}
