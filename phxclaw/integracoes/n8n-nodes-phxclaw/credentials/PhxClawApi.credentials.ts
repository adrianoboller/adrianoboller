import type {
  IAuthenticateGeneric,
  ICredentialTestRequest,
  ICredentialType,
  INodeProperties,
} from "n8n-workflow";

/**
 * Credencial do PhxClaw: a URL do `phxclaw servir` e o Bearer da API de tarefas (o token
 * de `phxclaw api chave`, ou o de `<pasta>/api.token`). O mesmo Bearer vale para o
 * endpoint MCP (`POST /mcp`), que e como a operacao "Run Tool" chama uma ferramenta.
 */
export class PhxClawApi implements ICredentialType {
  name = "phxClawApi";
  displayName = "PhxClaw API";
  documentationUrl = "https://github.com/adrianoboller/adrianoboller/blob/main/phxclaw/docs/N8N.md";
  properties: INodeProperties[] = [
    {
      displayName: "Base URL",
      name: "baseUrl",
      type: "string",
      default: "http://127.0.0.1:8787",
      placeholder: "http://127.0.0.1:8787",
      description: "Where `phxclaw servir` listens (no trailing slash)",
    },
    {
      displayName: "API Token",
      name: "token",
      type: "string",
      typeOptions: { password: true },
      default: "",
      description: "Bearer token of the PhxClaw task API (`phxclaw api chave`)",
    },
  ];

  authenticate: IAuthenticateGeneric = {
    type: "generic",
    properties: {
      headers: {
        Authorization: "=Bearer {{$credentials.token}}",
      },
    },
  };

  test: ICredentialTestRequest = {
    request: {
      baseURL: "={{$credentials.baseUrl}}",
      url: "/v1/tasks",
      method: "GET",
    },
  };
}
