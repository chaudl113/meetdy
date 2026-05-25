import type { AppSettings } from "@/bindings";

const APPLE_PROVIDER_ID = "apple_intelligence";

/**
 * Returns true when the user has finished configuring the AI post-processing
 * feature: post-processing is enabled, a provider is selected, and that
 * provider has the credentials it needs (an API key, or none for Apple
 * Intelligence which runs locally).
 *
 * Used to gate AI-only UI such as the Post Process settings section and the
 * meeting summary panel.
 */
export const isAiConfigured = (
  settings: AppSettings | null | undefined,
): boolean => {
  if (!settings) return false;
  if (!settings.post_process_enabled) return false;

  const providers = settings.post_process_providers ?? [];
  if (providers.length === 0) return false;

  const providerId =
    settings.post_process_provider_id || providers[0]?.id || "";
  if (!providerId) return false;

  if (providerId === APPLE_PROVIDER_ID) return true;

  const apiKey = settings.post_process_api_keys?.[providerId] ?? "";
  return apiKey.trim().length > 0;
};
