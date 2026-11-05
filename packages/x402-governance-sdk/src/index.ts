export type DecisionOutcome = "allow" | "deny" | "approval_required";

export type Decision = {
  decision: DecisionOutcome;
  decision_id?: string;
  reason_code?: string;
};

export type GuardrailClientOptions = {
  baseUrl: string;
  credential: string;
};

export class GuardrailClient {
  constructor(readonly options: GuardrailClientOptions) {}
}
