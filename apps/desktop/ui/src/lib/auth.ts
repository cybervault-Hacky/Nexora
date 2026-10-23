import { invoke } from "@tauri-apps/api/core";
import { isDesktopShell } from "./ipc";

export interface AuthUser {
  id: string;
  name: string;
  email: string;
  emailVerified: boolean;
}

export interface AccountProfile {
  id: string;
  name: string;
  email: string;
  emailVerified: boolean;
  hasPassword: boolean;
  providers: string[];
  planId: string;
  planName: string;
  createdAt: string;
}

export interface AuthSessionInfo {
  user: AuthUser | null;
  authenticated: boolean;
}

export interface AuthStatusResponse {
  status: string;
  message: string;
  emailDelivery?: string;
}

export interface AuthError {
  code: string;
  message: string;
  retryAfterSeconds?: number;
}

export interface LoginPayload {
  email: string;
  password: string;
}

export interface RegisterPayload {
  name: string;
  email: string;
  password: string;
}

export interface VerifyEmailPayload {
  token: string;
}

export interface ResendVerificationPayload {
  email: string;
}

export interface RecoveryPayload {
  email: string;
}

export interface ResetPasswordPayload {
  token: string;
  password: string;
}

export interface ChangePasswordPayload {
  currentPassword: string;
  newPassword: string;
}

export interface UpdateProfilePayload {
  name: string;
}

export interface DeleteAccountPayload {
  confirmEmail: string;
  currentPassword?: string;
}

export interface PlanPriceView {
  id: string;
  planId: string;
  interval: "month" | "year" | string;
  currency: string;
  amountMinor: number;
}

export interface PlanView {
  id: string;
  name: string;
  description: string;
  features: string[];
  modelQuotas: Record<string, number>;
  prices: PlanPriceView[];
}

export interface PlanCatalog {
  plans: PlanView[];
  paymentConfigured: boolean;
  provider: string;
}

export interface SubscriptionView {
  id: string;
  planId: string;
  status: string;
  currentPeriodStart: string;
  currentPeriodEnd: string;
  cancelAtPeriodEnd: boolean;
  canceledAt: string | null;
  provider: string;
}

export interface PaymentOrderView {
  id: string;
  planId: string;
  amountMinor: number;
  currency: string;
  status: string;
  createdAt: string;
}

export interface BillingStatus {
  effectivePlanId: string;
  effectivePlanName: string;
  subscription: SubscriptionView | null;
  recentOrders: PaymentOrderView[];
  paymentConfigured: boolean;
  portalAvailable: boolean;
}

export interface CheckoutPayload {
  planId: string;
  interval: string;
  idempotencyKey: string;
}

export interface CheckoutSession {
  orderId: string;
  checkoutUrl: string;
  providerSessionId: string;
  expiresNote: string;
}

export interface ModelUsageItem {
  modelId: string;
  dailyLimit: number;
  used: number;
  reserved: number;
  remaining: number;
}

export interface ServerUsageSummary {
  planId: string;
  planName: string;
  periodDayUtc: string;
  resetsAtUtc: string;
  models: ModelUsageItem[];
  offlineFallback?: boolean;
}

export interface DeveloperDiagnostics {
  platform: string;
  arch: string;
  appVersion: string;
  credentialVault: string;
  authServiceUrl: string;
  cspEnforced: boolean;
  offlineCapable: boolean;
}

function desktopRequiredError(feature = "authentication"): AuthError {
  return {
    code: "desktop_required",
    message: `Desktop app required for ${feature}.`,
  };
}

export async function authGetSession(): Promise<AuthSessionInfo> {
  if (!isDesktopShell()) {
    return { user: null, authenticated: false };
  }
  return await invoke<AuthSessionInfo>("auth_get_session");
}

export async function authRefreshSession(): Promise<AuthSessionInfo> {
  if (!isDesktopShell()) {
    return { user: null, authenticated: false };
  }
  return await invoke<AuthSessionInfo>("auth_refresh_session");
}

export async function authLogin(payload: LoginPayload): Promise<AuthUser> {
  if (!isDesktopShell()) {
    throw desktopRequiredError();
  }
  return await invoke<AuthUser>("auth_login", { request: payload });
}

export async function authRegister(payload: RegisterPayload): Promise<AuthStatusResponse> {
  if (!isDesktopShell()) {
    throw desktopRequiredError();
  }
  return await invoke<AuthStatusResponse>("auth_register", { request: payload });
}

export async function authVerifyEmail(payload: VerifyEmailPayload): Promise<AuthStatusResponse> {
  if (!isDesktopShell()) {
    throw desktopRequiredError();
  }
  return await invoke<AuthStatusResponse>("auth_verify_email", { request: payload });
}

export async function authResendVerification(payload: ResendVerificationPayload): Promise<AuthStatusResponse> {
  if (!isDesktopShell()) {
    throw desktopRequiredError();
  }
  return await invoke<AuthStatusResponse>("auth_resend_verification", { request: payload });
}

export async function authRecoverPassword(payload: RecoveryPayload): Promise<AuthStatusResponse> {
  if (!isDesktopShell()) {
    throw desktopRequiredError();
  }
  return await invoke<AuthStatusResponse>("auth_recover_password", { request: payload });
}

export async function authResetPassword(payload: ResetPasswordPayload): Promise<AuthStatusResponse> {
  if (!isDesktopShell()) {
    throw desktopRequiredError();
  }
  return await invoke<AuthStatusResponse>("auth_reset_password", { request: payload });
}

export async function authLogout(): Promise<boolean> {
  if (!isDesktopShell()) {
    return true;
  }
  return await invoke<boolean>("auth_logout");
}

export async function authGoogleBegin(): Promise<AuthUser> {
  if (!isDesktopShell()) {
    throw desktopRequiredError("Google authentication");
  }
  return await invoke<AuthUser>("auth_google_begin");
}

export async function authGoogleCancel(): Promise<boolean> {
  if (!isDesktopShell()) {
    return true;
  }
  return await invoke<boolean>("auth_google_cancel");
}

export async function authChangePassword(payload: ChangePasswordPayload): Promise<AuthStatusResponse> {
  if (!isDesktopShell()) {
    throw desktopRequiredError("password management");
  }
  return await invoke<AuthStatusResponse>("auth_change_password", { request: payload });
}

export async function getDeveloperDiagnostics(): Promise<DeveloperDiagnostics> {
  if (!isDesktopShell()) {
    return {
      platform: "browser-preview",
      arch: "web",
      appVersion: "0.1.0",
      credentialVault: "Unavailable in browser preview",
      authServiceUrl: "http://127.0.0.1:8787",
      cspEnforced: true,
      offlineCapable: false,
    };
  }
  return await invoke<DeveloperDiagnostics>("get_developer_diagnostics");
}

export async function accountGetProfile(): Promise<AccountProfile> {
  if (!isDesktopShell()) {
    throw desktopRequiredError("account profile");
  }
  return await invoke<AccountProfile>("account_get_profile");
}

export async function accountUpdateProfile(payload: UpdateProfilePayload): Promise<AccountProfile> {
  if (!isDesktopShell()) {
    throw desktopRequiredError("profile updates");
  }
  return await invoke<AccountProfile>("account_update_profile", { request: payload });
}

export async function accountLinkGoogle(): Promise<AccountProfile> {
  if (!isDesktopShell()) {
    throw desktopRequiredError("Google account linking");
  }
  return await invoke<AccountProfile>("account_link_google");
}

export async function accountExportData(): Promise<Record<string, unknown>> {
  if (!isDesktopShell()) {
    throw desktopRequiredError("data export");
  }
  return await invoke<Record<string, unknown>>("account_export_data");
}

export async function accountDelete(payload: DeleteAccountPayload): Promise<boolean> {
  if (!isDesktopShell()) {
    throw desktopRequiredError("account deletion");
  }
  return await invoke<boolean>("account_delete", { request: payload });
}

export async function billingListPlans(): Promise<PlanCatalog> {
  if (!isDesktopShell()) {
    return {
      paymentConfigured: false,
      provider: "stripe",
      plans: [
        {
          id: "free",
          name: "Free",
          description: "Daily local AI coding assistance with Nexora Mini and Sonnet.",
          features: [
            "Local GGUF inference on your hardware",
            "25 Nexora Mini messages per day",
            "5 Nexora Sonnet messages per day",
            "Workspace context, symbol index & diff preview",
          ],
          modelQuotas: { "nexora-mini": 25, "nexora-sonnet": 5 },
          prices: [{ id: "free_monthly", planId: "free", interval: "month", currency: "usd", amountMinor: 0 }],
        },
        {
          id: "pro",
          name: "Pro",
          description: "Higher daily model quotas and priority updates for individual developers.",
          features: [
            "Everything in Free",
            "300 Nexora Mini messages per day",
            "100 Nexora Sonnet messages per day",
            "Multi-file patch batches & verification workflows",
          ],
          modelQuotas: { "nexora-mini": 300, "nexora-sonnet": 100 },
          prices: [
            { id: "pro_monthly", planId: "pro", interval: "month", currency: "usd", amountMinor: 1900 },
            { id: "pro_yearly", planId: "pro", interval: "year", currency: "usd", amountMinor: 19000 },
          ],
        },
        {
          id: "team",
          name: "Team",
          description: "High-volume daily quotas and commercial seat management for engineering teams.",
          features: [
            "Everything in Pro",
            "1,000 Nexora Mini messages per day",
            "400 Nexora Sonnet messages per day",
            "Centralized billing & audit trail",
          ],
          modelQuotas: { "nexora-mini": 1000, "nexora-sonnet": 400 },
          prices: [
            { id: "team_monthly", planId: "team", interval: "month", currency: "usd", amountMinor: 3900 },
            { id: "team_yearly", planId: "team", interval: "year", currency: "usd", amountMinor: 39000 },
          ],
        },
      ],
    };
  }
  return await invoke<PlanCatalog>("billing_list_plans");
}

export async function billingGetStatus(): Promise<BillingStatus> {
  if (!isDesktopShell()) {
    throw desktopRequiredError("subscription status");
  }
  return await invoke<BillingStatus>("billing_get_status");
}

export async function billingCreateCheckout(payload: CheckoutPayload): Promise<CheckoutSession> {
  if (!isDesktopShell()) {
    throw desktopRequiredError("checkout");
  }
  return await invoke<CheckoutSession>("billing_create_checkout", { request: payload });
}

export async function billingVerifyOrder(orderId: string): Promise<BillingStatus> {
  if (!isDesktopShell()) {
    throw desktopRequiredError("payment verification");
  }
  return await invoke<BillingStatus>("billing_verify_order", { request: { orderId } });
}

export async function billingCancelSubscription(immediate: boolean): Promise<BillingStatus> {
  if (!isDesktopShell()) {
    throw desktopRequiredError("subscription cancellation");
  }
  return await invoke<BillingStatus>("billing_cancel_subscription", { request: { immediate } });
}

export async function billingChangePlan(planId: string, interval: string): Promise<BillingStatus> {
  if (!isDesktopShell()) {
    throw desktopRequiredError("plan change");
  }
  return await invoke<BillingStatus>("billing_change_plan", { request: { planId, interval } });
}

export async function billingOpenPortal(): Promise<{ portalUrl: string }> {
  if (!isDesktopShell()) {
    throw desktopRequiredError("billing portal");
  }
  return await invoke<{ portalUrl: string }>("billing_open_portal");
}

export async function usageGetServerSummary(): Promise<ServerUsageSummary> {
  if (!isDesktopShell()) {
    throw desktopRequiredError("usage summary");
  }
  return await invoke<ServerUsageSummary>("usage_get_server_summary");
}
