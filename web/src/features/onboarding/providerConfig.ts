import { useEffect, useRef, useState } from "react";
import type { TFunction } from "i18next";
import { ApiRequestError } from "../../lib/api/client";
import type {
  SaveOnboardingPreferencesRequest,
  SaveOnboardingProviderRequest,
} from "./api";


export const PROVIDER_KIND_SUGGESTIONS = [
  "openai",
  "volcengine",
  "deepseek",
  "anthropic",
  "openrouter",
  "ollama",
  "lmstudio",
] as const;

export const PERSONALITY_OPTIONS = [
  "calm_engineering",
  "friendly_collab",
  "autonomous_executor",
] as const;

export const MEMORY_PROFILE_OPTIONS = [
  "window_only",
  "window_plus_summary",
  "profile_plus_window",
] as const;


export interface ProviderConfigFormSource {
  kind: string;
  model: string;
  baseUrlOrEndpoint: string;
  apiKeyConfigured: boolean;
}

interface FormResetOptions {
  force?: boolean;
}

export function useProviderConfigForm(source: ProviderConfigFormSource) {
  const sourceRef = useRef(source);
  const [kind, setKind] = useState(source.kind);
  const [model, setModel] = useState(source.model);
  const [baseUrlOrEndpoint, setBaseUrlOrEndpoint] = useState(source.baseUrlOrEndpoint);
  const [apiKey, setApiKey] = useState("");
  const [apiKeyDirty, setApiKeyDirty] = useState(false);
  const [kindDirty, setKindDirty] = useState(false);
  const [modelDirty, setModelDirty] = useState(false);
  const [baseUrlDirty, setBaseUrlDirty] = useState(false);

  const apiKeyDirtyRef = useRef(false);
  const kindDirtyRef = useRef(false);
  const modelDirtyRef = useRef(false);
  const baseUrlDirtyRef = useRef(false);

  function updateApiKeyDirty(nextDirty: boolean) {
    apiKeyDirtyRef.current = nextDirty;
    setApiKeyDirty(nextDirty);
  }

  function updateKindDirty(nextDirty: boolean) {
    kindDirtyRef.current = nextDirty;
    setKindDirty(nextDirty);
  }

  function updateModelDirty(nextDirty: boolean) {
    modelDirtyRef.current = nextDirty;
    setModelDirty(nextDirty);
  }

  function updateBaseUrlDirty(nextDirty: boolean) {
    baseUrlDirtyRef.current = nextDirty;
    setBaseUrlDirty(nextDirty);
  }

  function resetFromSource(
    nextSource: ProviderConfigFormSource,
    options?: FormResetOptions,
  ) {
    const force = options?.force ?? false;
    sourceRef.current = nextSource;

    if (force || !kindDirtyRef.current) {
      setKind(nextSource.kind);
      if (force) {
        updateKindDirty(false);
      }
    }

    if (force || !modelDirtyRef.current) {
      setModel(nextSource.model);
      if (force) {
        updateModelDirty(false);
      }
    }

    if (force || !baseUrlDirtyRef.current) {
      setBaseUrlOrEndpoint(nextSource.baseUrlOrEndpoint);
      if (force) {
        updateBaseUrlDirty(false);
      }
    }

    if (force || !apiKeyDirtyRef.current) {
      setApiKey("");
      updateApiKeyDirty(false);
    }
  }

  useEffect(() => {
    resetFromSource(source);
  }, [source.baseUrlOrEndpoint, source.kind, source.model]);

  function setKindWithRouteReset(nextKind: string) {
    setKind(nextKind);
    updateKindDirty(nextKind !== sourceRef.current.kind);
    setBaseUrlOrEndpoint((current) => {
      const nextValue =
        current === sourceRef.current.baseUrlOrEndpoint ? "" : current;
      updateBaseUrlDirty(nextValue !== sourceRef.current.baseUrlOrEndpoint);
      return nextValue;
    });
  }

  function setModelValue(nextModel: string) {
    setModel(nextModel);
    updateModelDirty(nextModel !== sourceRef.current.model);
  }

  function setBaseUrlValue(nextBaseUrlOrEndpoint: string) {
    setBaseUrlOrEndpoint(nextBaseUrlOrEndpoint);
    updateBaseUrlDirty(nextBaseUrlOrEndpoint !== sourceRef.current.baseUrlOrEndpoint);
  }

  function setApiKeyValue(nextApiKey: string) {
    setApiKey(nextApiKey);
    updateApiKeyDirty(true);
  }

  function handleApiKeyFocus() {
    if (sourceRef.current.apiKeyConfigured && !apiKeyDirtyRef.current) {
      setApiKey("");
      updateApiKeyDirty(true);
    }
  }

  function markApiKeyPristine() {
    setApiKey("");
    updateApiKeyDirty(false);
  }

  return {
    kind,
    model,
    baseUrlOrEndpoint,
    apiKey,
    apiKeyDirty,
    isDirty: kindDirty || modelDirty || baseUrlDirty || apiKeyDirty,
    resetFromSource,
    setModel: setModelValue,
    setBaseUrlOrEndpoint: setBaseUrlValue,
    setKindWithRouteReset,
    setApiKeyValue,
    handleApiKeyFocus,
    markApiKeyPristine,
  };
}

export function buildProviderSavePayload(input: {
  kind: string;
  model: string;
  baseUrlOrEndpoint: string;
  apiKey: string;
}): SaveOnboardingProviderRequest {
  const payload: SaveOnboardingProviderRequest = {
    kind: input.kind.trim(),
    model: input.model.trim(),
    baseUrlOrEndpoint: input.baseUrlOrEndpoint.trim(),
  };

  const normalizedApiKey = input.apiKey.trim();
  if (normalizedApiKey) {
    payload.apiKey = normalizedApiKey;
  }

  return payload;
}

export interface PreferencesFormSource {
  personality: string;
  memoryProfile: string;
  promptAddendum: string;
}

export function usePreferencesForm(source: PreferencesFormSource) {
  const sourceRef = useRef(source);
  const [personality, setPersonality] = useState(source.personality);
  const [memoryProfile, setMemoryProfile] = useState(source.memoryProfile);
  const [promptAddendum, setPromptAddendum] = useState(source.promptAddendum);
  const [personalityDirty, setPersonalityDirty] = useState(false);
  const [memoryProfileDirty, setMemoryProfileDirty] = useState(false);
  const [promptAddendumDirty, setPromptAddendumDirty] = useState(false);

  const personalityDirtyRef = useRef(false);
  const memoryProfileDirtyRef = useRef(false);
  const promptAddendumDirtyRef = useRef(false);

  function updatePersonalityDirty(nextDirty: boolean) {
    personalityDirtyRef.current = nextDirty;
    setPersonalityDirty(nextDirty);
  }

  function updateMemoryProfileDirty(nextDirty: boolean) {
    memoryProfileDirtyRef.current = nextDirty;
    setMemoryProfileDirty(nextDirty);
  }

  function updatePromptAddendumDirty(nextDirty: boolean) {
    promptAddendumDirtyRef.current = nextDirty;
    setPromptAddendumDirty(nextDirty);
  }

  function resetFromSource(
    nextSource: PreferencesFormSource,
    options?: FormResetOptions,
  ) {
    const force = options?.force ?? false;
    sourceRef.current = nextSource;

    if (force || !personalityDirtyRef.current) {
      setPersonality(nextSource.personality);
      if (force) {
        updatePersonalityDirty(false);
      }
    }

    if (force || !memoryProfileDirtyRef.current) {
      setMemoryProfile(nextSource.memoryProfile);
      if (force) {
        updateMemoryProfileDirty(false);
      }
    }

    if (force || !promptAddendumDirtyRef.current) {
      setPromptAddendum(nextSource.promptAddendum);
      if (force) {
        updatePromptAddendumDirty(false);
      }
    }
  }

  useEffect(() => {
    resetFromSource(source);
  }, [source.memoryProfile, source.personality, source.promptAddendum]);

  function setPersonalityValue(nextPersonality: string) {
    setPersonality(nextPersonality);
    updatePersonalityDirty(nextPersonality !== sourceRef.current.personality);
  }

  function setMemoryProfileValue(nextMemoryProfile: string) {
    setMemoryProfile(nextMemoryProfile);
    updateMemoryProfileDirty(nextMemoryProfile !== sourceRef.current.memoryProfile);
  }

  function setPromptAddendumValue(nextPromptAddendum: string) {
    setPromptAddendum(nextPromptAddendum);
    updatePromptAddendumDirty(
      nextPromptAddendum !== sourceRef.current.promptAddendum,
    );
  }

  return {
    personality,
    memoryProfile,
    promptAddendum,
    isDirty: personalityDirty || memoryProfileDirty || promptAddendumDirty,
    resetFromSource,
    setPersonality: setPersonalityValue,
    setMemoryProfile: setMemoryProfileValue,
    setPromptAddendum: setPromptAddendumValue,
  };
}

export function buildPreferencesSavePayload(input: {
  personality: string;
  memoryProfile: string;
  promptAddendum: string;
}): SaveOnboardingPreferencesRequest {
  const payload: SaveOnboardingPreferencesRequest = {
    personality: input.personality,
    memoryProfile: input.memoryProfile,
  };

  const normalizedPromptAddendum = input.promptAddendum.trim();
  if (normalizedPromptAddendum) {
    payload.promptAddendum = normalizedPromptAddendum;
  }

  return payload;
}

export function readProviderValidationFailure(

  credentialStatus: string,
  t: TFunction,
): string {
  return t(`onboarding.validation.statuses.${credentialStatus}`, {
    defaultValue: t("onboarding.validation.failed"),
  });
}

export function readProviderSaveError(
  error: unknown,
  t: TFunction,
  fallbackKey: string,
): string {
  if (error instanceof ApiRequestError) {
    return error.message;
  }
  return t(fallbackKey);
}
