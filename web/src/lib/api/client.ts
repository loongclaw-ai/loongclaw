import { getApiBaseUrl } from "../config/env";
import { getStoredToken } from "../auth/tokenStore";

export class ApiRequestError extends Error {
  status: number;
  code?: string;
  method: string;
  url: string;

  constructor(
    message: string,
    status: number,
    code: string | undefined,
    method: string,
    url: string,
  ) {
    super(message);
    this.name = "ApiRequestError";
    this.status = status;
    this.code = code;
    this.method = method;
    this.url = url;
  }
}

export async function apiGet<T>(path: string): Promise<T> {
  return apiRequest<T>(path);
}

export async function apiPost<TResponse, TBody>(
  path: string,
  body: TBody,
): Promise<TResponse> {
  return apiRequest<TResponse>(path, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  });
}

export async function apiDelete(path: string): Promise<void> {
  await apiRequest<void>(path, {
    method: "DELETE",
  });
}

export async function apiOpenStream(
  path: string,
  init?: RequestInit,
): Promise<Response> {
  return apiFetch(path, init);
}

async function apiRequest<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await apiFetch(path, init);
  const payload = await response.json().catch(() => null);
  return payload as T;
}

function extractApiErrorMessage(payload: unknown): string | null {
  return typeof payload === "object" &&
    payload !== null &&
    "error" in payload &&
    typeof payload.error === "object" &&
    payload.error !== null &&
    "message" in payload.error &&
    typeof payload.error.message === "string"
    ? payload.error.message.trim()
    : null;
}

function extractApiErrorCode(payload: unknown): string | undefined {
  return typeof payload === "object" &&
    payload !== null &&
    "error" in payload &&
    typeof payload.error === "object" &&
    payload.error !== null &&
    "code" in payload.error &&
    typeof payload.error.code === "string"
    ? payload.error.code
    : undefined;
}

function describeApiFailure(
  status: number,
  payload: unknown,
  method: string,
  url: string,
): string {
  const payloadMessage = extractApiErrorMessage(payload);

  if (payloadMessage) {
    return payloadMessage;
  }

  const requestTarget = `${method} ${url}`;
  if (status === 404) {
    return `Request failed: 404 (${requestTarget}). Check that the Web API is running and that the browser is pointing to the correct API base URL.`;
  }

  if (status === 0) {
    return `Unable to reach the Web API (${requestTarget}). Check that the API is running and reachable from this browser.`;
  }

  return `Request failed: ${status} (${requestTarget})`;
}

async function apiFetch(path: string, init?: RequestInit): Promise<Response> {
  const headers = new Headers(init?.headers);
  const token = getStoredToken();
  if (token && !headers.has("Authorization")) {
    headers.set("Authorization", `Bearer ${token}`);
  }

  const method = (init?.method ?? "GET").toUpperCase();
  const url = `${getApiBaseUrl()}${path}`;

  let response: Response;
  try {
    response = await fetch(url, {
      ...init,
      credentials: "include",
      headers,
    });
  } catch {
    throw new ApiRequestError(
      describeApiFailure(0, null, method, url),
      0,
      undefined,
      method,
      url,
    );
  }

  if (!response.ok) {
    const payload = await response.json().catch(() => null);
    throw new ApiRequestError(
      describeApiFailure(response.status, payload, method, url),
      response.status,
      extractApiErrorCode(payload),
      method,
      url,
    );
  }

  return response;
}
