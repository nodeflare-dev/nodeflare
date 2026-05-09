const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || '';

export interface AuthResponse {
  access_token: string;
  refresh_token?: string;
  token_type: string;
  expires_in: number;
  user: {
    id: string;
    email: string;
    name: string;
    avatar_url?: string;
    created_at: string;
  };
}

export interface RegisterResponse {
  message: string;
  email: string;
}

export interface VerifyEmailResponse {
  message: string;
  verified: boolean;
}

export interface ForgotPasswordResponse {
  message: string;
}

export interface ResetPasswordResponse {
  message: string;
}

export interface ApiError {
  message: string;
  code?: string;
}

async function handleResponse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const errorText = await response.text();
    let errorMessage = errorText;
    try {
      const errorJson = JSON.parse(errorText);
      errorMessage = errorJson.message || errorJson.error || errorText;
    } catch {
      // Keep the plain text error
    }
    throw new Error(errorMessage);
  }
  return response.json();
}

/**
 * Register a new user with email and password
 */
export async function register(
  email: string,
  password: string,
  name: string
): Promise<RegisterResponse> {
  const response = await fetch(`${API_BASE_URL}/api/v1/auth/register`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ email, password, name }),
  });

  return handleResponse<RegisterResponse>(response);
}

/**
 * Login with email and password
 */
export async function login(email: string, password: string): Promise<AuthResponse> {
  const response = await fetch(`${API_BASE_URL}/api/v1/auth/login`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    credentials: 'include',
    body: JSON.stringify({ email, password }),
  });

  return handleResponse<AuthResponse>(response);
}

/**
 * Verify email with token
 */
export async function verifyEmail(token: string): Promise<VerifyEmailResponse> {
  const response = await fetch(
    `${API_BASE_URL}/api/v1/auth/verify-email?token=${encodeURIComponent(token)}`,
    {
      method: 'GET',
      credentials: 'include',
    }
  );

  return handleResponse<VerifyEmailResponse>(response);
}

/**
 * Request password reset email
 */
export async function forgotPassword(email: string): Promise<ForgotPasswordResponse> {
  const response = await fetch(`${API_BASE_URL}/api/v1/auth/forgot-password`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ email }),
  });

  return handleResponse<ForgotPasswordResponse>(response);
}

/**
 * Reset password with token
 */
export async function resetPassword(
  token: string,
  password: string
): Promise<ResetPasswordResponse> {
  const response = await fetch(`${API_BASE_URL}/api/v1/auth/reset-password`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ token, password }),
  });

  return handleResponse<ResetPasswordResponse>(response);
}

/**
 * Resend verification email
 */
export async function resendVerification(email: string): Promise<RegisterResponse> {
  const response = await fetch(`${API_BASE_URL}/api/v1/auth/resend-verification`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ email }),
  });

  return handleResponse<RegisterResponse>(response);
}
