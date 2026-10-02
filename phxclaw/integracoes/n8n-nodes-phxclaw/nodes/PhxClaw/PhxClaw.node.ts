import type {
  IDataObject,
  IExecuteFunctions,
  IHttpRequestMethods,
  IHttpRequestOptions,
  INodeExecutionData,
  INodeType,
  INodeTypeDescription,
} from "n8n-workflow";
import { NodeApiError, NodeOperationError } from "n8n-workflow";

/**
 * O no do PhxClaw no n8n: cada operacao e UMA rota da API de tarefas (`/v1/tasks`) ou o
 * endpoint MCP (`POST /mcp`) do `phxclaw servir`. Nada aqui reimplementa o agente: o no
 * so monta o pedido e devolve o JSON que o servidor responde.
 *
 * Estados finais que o "Wait for Result" reconhece sao os do `TaskStatus` do servidor
 * (snake_case): `completed`, `failed`, `cancelled`; e os de espera por humano
 * (`awaiting_approval`, `awaiting_input`) tambem encerram a espera, porque so outro no
 * ("Approve Plan", "Answer Question") os destrava.
 */
const ESTADOS_QUE_PARAM = new Set([
  "completed",
  "failed",
  "cancelled",
  "awaiting_approval",
  "awaiting_input",
]);

async function pedir(
  ctx: IExecuteFunctions,
  method: IHttpRequestMethods,
  caminho: string,
  body?: IDataObject,
): Promise<IDataObject> {
  const cred = (await ctx.getCredentials("phxClawApi")) as { baseUrl: string };
  const opcoes: IHttpRequestOptions = {
    method,
    url: `${cred.baseUrl.replace(/\/+$/, "")}${caminho}`,
    json: true,
    headers: { Accept: "application/json" },
  };
  if (body !== undefined) {
    opcoes.body = body;
  }
  return (await ctx.helpers.httpRequestWithAuthentication.call(
    ctx,
    "phxClawApi",
    opcoes,
  )) as IDataObject;
}

function dormir(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function esperar(
  ctx: IExecuteFunctions,
  id: string,
  prazoSeg: number,
  intervaloSeg: number,
): Promise<IDataObject> {
  const fim = Date.now() + prazoSeg * 1000;
  for (;;) {
    const t = await pedir(ctx, "GET", `/v1/tasks/${encodeURIComponent(id)}`);
    if (ESTADOS_QUE_PARAM.has(String(t.status))) {
      return t;
    }
    if (Date.now() >= fim) {
      return { ...t, timed_out: true };
    }
    await dormir(Math.max(1, intervaloSeg) * 1000);
  }
}

export class PhxClaw implements INodeType {
  description: INodeTypeDescription = {
    displayName: "PhxClaw",
    name: "phxClaw",
    icon: "file:phxclaw.svg",
    group: ["transform"],
    version: 1,
    subtitle: '={{$parameter["operation"]}}',
    description: "Create and follow PhxClaw agent tasks, answer questions, approve plans and run tools",
    defaults: { name: "PhxClaw" },
    inputs: ["main"],
    outputs: ["main"],
    credentials: [{ name: "phxClawApi", required: true }],
    properties: [
      {
        displayName: "Operation",
        name: "operation",
        type: "options",
        noDataExpression: true,
        options: [
          { name: "Create Task", value: "createTask", action: "Create a task", description: "POST /v1/tasks" },
          { name: "Wait for Result", value: "waitTask", action: "Wait for a task result", description: "Poll GET /v1/tasks/{id} until a final state" },
          { name: "Answer Question", value: "answer", action: "Answer a task question", description: "POST /v1/tasks/{id}/answer" },
          { name: "Approve Plan", value: "approve", action: "Approve a task plan", description: "POST /v1/tasks/{id}/approve" },
          { name: "Run Tool", value: "runTool", action: "Run an agent tool", description: "tools/call on POST /mcp (streamable HTTP)" },
        ],
        default: "createTask",
      },
      // --- createTask
      {
        displayName: "Objective",
        name: "objective",
        type: "string",
        typeOptions: { rows: 4 },
        default: "",
        required: true,
        displayOptions: { show: { operation: ["createTask"] } },
        description: "What the agent must do (up to 20000 characters)",
      },
      {
        displayName: "Model",
        name: "model",
        type: "string",
        default: "",
        displayOptions: { show: { operation: ["createTask"] } },
        description: 'Model spec such as "ollama:qwen2.5:1.5b"; empty = server default',
      },
      {
        displayName: "Plan First",
        name: "planFirst",
        type: "boolean",
        default: false,
        displayOptions: { show: { operation: ["createTask"] } },
        description: "Whether to generate a plan and wait for approval before executing",
      },
      {
        displayName: "Webhook URL",
        name: "webhook",
        type: "string",
        default: "",
        displayOptions: { show: { operation: ["createTask"] } },
        description: "URL the server POSTs to when the task finishes (must be allowed by api.webhook_origens)",
      },
      {
        displayName: "Wait for Result",
        name: "wait",
        type: "boolean",
        default: true,
        displayOptions: { show: { operation: ["createTask"] } },
        description: "Whether to poll the task until it reaches a final state",
      },
      // --- waitTask / answer / approve
      {
        displayName: "Task ID",
        name: "taskId",
        type: "string",
        default: "",
        required: true,
        displayOptions: { show: { operation: ["waitTask", "answer", "approve"] } },
      },
      {
        displayName: "Timeout (seconds)",
        name: "timeout",
        type: "number",
        default: 300,
        displayOptions: { show: { operation: ["createTask", "waitTask"] } },
        description: "How long to wait before returning the task as is (timed_out: true)",
      },
      {
        displayName: "Poll Interval (seconds)",
        name: "interval",
        type: "number",
        default: 2,
        displayOptions: { show: { operation: ["createTask", "waitTask"] } },
      },
      {
        displayName: "Answer",
        name: "answer",
        type: "string",
        typeOptions: { rows: 2 },
        default: "",
        required: true,
        displayOptions: { show: { operation: ["answer"] } },
        description: "The reply to the question the task asked (status awaiting_input)",
      },
      // --- runTool
      {
        displayName: "Tool Name",
        name: "toolName",
        type: "string",
        default: "",
        required: true,
        displayOptions: { show: { operation: ["runTool"] } },
        description: "A tool the server policy grants (see tools/list on /mcp)",
      },
      {
        displayName: "Arguments (JSON)",
        name: "arguments",
        type: "json",
        default: "{}",
        displayOptions: { show: { operation: ["runTool"] } },
      },
    ],
  };

  async execute(this: IExecuteFunctions): Promise<INodeExecutionData[][]> {
    const itens = this.getInputData();
    const saida: INodeExecutionData[] = [];
    const operation = this.getNodeParameter("operation", 0) as string;

    for (let i = 0; i < itens.length; i++) {
      try {
        let resultado: IDataObject;
        switch (operation) {
          case "createTask": {
            const corpo: IDataObject = {
              objective: this.getNodeParameter("objective", i) as string,
              plan_first: this.getNodeParameter("planFirst", i) as boolean,
            };
            const model = this.getNodeParameter("model", i) as string;
            if (model.trim() !== "") {
              corpo.model = model.trim();
            }
            const webhook = this.getNodeParameter("webhook", i) as string;
            if (webhook.trim() !== "") {
              corpo.webhook = webhook.trim();
            }
            const criada = await pedir(this, "POST", "/v1/tasks", corpo);
            const id = String(criada.id);
            resultado = (this.getNodeParameter("wait", i) as boolean)
              ? await esperar(
                  this,
                  id,
                  this.getNodeParameter("timeout", i) as number,
                  this.getNodeParameter("interval", i) as number,
                )
              : criada;
            break;
          }
          case "waitTask":
            resultado = await esperar(
              this,
              this.getNodeParameter("taskId", i) as string,
              this.getNodeParameter("timeout", i) as number,
              this.getNodeParameter("interval", i) as number,
            );
            break;
          case "answer":
            resultado = await pedir(
              this,
              "POST",
              `/v1/tasks/${encodeURIComponent(this.getNodeParameter("taskId", i) as string)}/answer`,
              { answer: this.getNodeParameter("answer", i) as string },
            );
            break;
          case "approve":
            resultado = await pedir(
              this,
              "POST",
              `/v1/tasks/${encodeURIComponent(this.getNodeParameter("taskId", i) as string)}/approve`,
            );
            break;
          case "runTool": {
            const bruto = this.getNodeParameter("arguments", i);
            const args =
              typeof bruto === "string" ? (JSON.parse(bruto) as IDataObject) : (bruto as IDataObject);
            // Uma mensagem JSON-RPC por pedido: o /mcp responde JSON, sem SSE.
            const r = await pedir(this, "POST", "/mcp", {
              jsonrpc: "2.0",
              id: i + 1,
              method: "tools/call",
              params: { name: this.getNodeParameter("toolName", i) as string, arguments: args },
            });
            if (r.error !== undefined) {
              throw new NodeOperationError(this.getNode(), `MCP: ${JSON.stringify(r.error)}`, { itemIndex: i });
            }
            const res = (r.result ?? {}) as IDataObject;
            const conteudo = (res.content ?? []) as Array<{ type: string; text?: string }>;
            resultado = {
              is_error: res.isError === true,
              text: conteudo
                .filter((c) => c.type === "text")
                .map((c) => c.text ?? "")
                .join("\n"),
            };
            break;
          }
          default:
            throw new NodeOperationError(this.getNode(), `operation desconhecida: ${operation}`, { itemIndex: i });
        }
        saida.push({ json: resultado, pairedItem: { item: i } });
      } catch (e) {
        if (this.continueOnFail()) {
          saida.push({ json: { error: (e as Error).message }, pairedItem: { item: i } });
          continue;
        }
        if (e instanceof NodeOperationError) {
          throw e;
        }
        throw new NodeApiError(this.getNode(), e as never, { itemIndex: i });
      }
    }
    return [saida];
  }
}
