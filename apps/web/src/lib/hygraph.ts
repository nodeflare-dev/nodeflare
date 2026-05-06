import 'server-only';
import sanitizeHtmlLib from 'sanitize-html';

const HYGRAPH_ENDPOINT = process.env.HYGRAPH_ENDPOINT || 'https://api-us-west-2.hygraph.com/v2/cmmky48hh00h006w5q885vkcf/master';
// SECURITY: Token must be provided via environment variable - never commit tokens to code
const HYGRAPH_TOKEN = process.env.HYGRAPH_TOKEN;

// Configure sanitize-html with safe defaults
const SANITIZE_CONFIG: sanitizeHtmlLib.IOptions = {
  allowedTags: [
    'h1', 'h2', 'h3', 'h4', 'h5', 'h6',
    'p', 'br', 'hr',
    'ul', 'ol', 'li',
    'blockquote', 'pre', 'code',
    'a', 'strong', 'em', 'u', 's', 'sub', 'sup',
    'table', 'thead', 'tbody', 'tr', 'th', 'td',
    'img', 'figure', 'figcaption',
    'div', 'span',
  ],
  allowedAttributes: {
    'a': ['href', 'title', 'target', 'rel'],
    'img': ['src', 'alt', 'title', 'width', 'height'],
    '*': ['class', 'id'],
  },
  disallowedTagsMode: 'discard',
};

export interface Author {
  id: string;
  name: string;
  bio?: string;
}

export interface Category {
  id: string;
  name: string;
  slug: string;
}

export interface BlogPost {
  id: string;
  title: string;
  slug: string;
  excerpt?: string;
  content?: {
    html: string;
    text: string;
  };
  publishDate?: string;
  author?: Author;
  categories: Category[];
}

// Scalability: Retry configuration for resilient API calls
const RETRY_CONFIG = {
  maxRetries: 3,
  initialDelayMs: 1000,
  maxDelayMs: 8000,
  backoffMultiplier: 2,
};

/**
 * Sleep for a specified duration
 */
function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * Check if an error is retryable (network errors, 5xx, 429)
 */
function isRetryableError(error: unknown, response?: Response): boolean {
  // Network errors are retryable
  if (error instanceof TypeError && error.message.includes('fetch')) {
    return true;
  }

  // Check HTTP status codes
  if (response) {
    // 5xx server errors are retryable
    if (response.status >= 500 && response.status < 600) {
      return true;
    }
    // 429 Too Many Requests is retryable
    if (response.status === 429) {
      return true;
    }
  }

  return false;
}

/**
 * Fetch from Hygraph with exponential backoff retry
 * Scalability: Handles transient network errors and rate limiting
 */
async function fetchHygraph<T>(query: string, variables?: Record<string, unknown>): Promise<T> {
  if (!HYGRAPH_TOKEN) {
    throw new Error('HYGRAPH_TOKEN is not configured');
  }

  let lastError: Error | null = null;
  let delay = RETRY_CONFIG.initialDelayMs;

  for (let attempt = 0; attempt <= RETRY_CONFIG.maxRetries; attempt++) {
    try {
      const controller = new AbortController();
      // Scalability: Add timeout to prevent hanging requests
      const timeoutId = setTimeout(() => controller.abort(), 30000);

      const res = await fetch(HYGRAPH_ENDPOINT, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${HYGRAPH_TOKEN}`,
        },
        body: JSON.stringify({ query, variables }),
        next: { revalidate: 60 },
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

      // Check if we should retry based on status code
      if (!res.ok && isRetryableError(null, res)) {
        if (attempt < RETRY_CONFIG.maxRetries) {
          await sleep(delay);
          delay = Math.min(delay * RETRY_CONFIG.backoffMultiplier, RETRY_CONFIG.maxDelayMs);
          continue;
        }
      }

      const json = await res.json();

      if (json.errors) {
        throw new Error(json.errors[0]?.message || 'GraphQL Error');
      }

      return json.data;
    } catch (error) {
      lastError = error instanceof Error ? error : new Error(String(error));

      // Check if error is retryable
      if (isRetryableError(error) && attempt < RETRY_CONFIG.maxRetries) {
        await sleep(delay);
        delay = Math.min(delay * RETRY_CONFIG.backoffMultiplier, RETRY_CONFIG.maxDelayMs);
        continue;
      }

      throw lastError;
    }
  }

  throw lastError || new Error('Hygraph request failed after retries');
}

/**
 * Sanitize HTML content to prevent XSS attacks.
 * Uses sanitize-html for server-side compatible sanitization.
 */
export function sanitizeHtml(html: string): string {
  if (!html) return '';
  return sanitizeHtmlLib(html, SANITIZE_CONFIG);
}

// Map next-intl locale to Hygraph locales with fallback
function toHygraphLocales(locale: string): string[] {
  // Always include 'en' as fallback since content may only exist in English
  return locale === 'ja' ? ['ja_JP', 'en'] : ['en'];
}

export async function getBlogPosts(locale: string = 'en'): Promise<BlogPost[]> {
  const query = `
    query GetBlogPosts($locales: [Locale!]!) {
      blogPosts(locales: $locales, orderBy: publishDate_DESC, stage: PUBLISHED) {
        id
        title
        slug
        excerpt
        publishDate
        author {
          id
          name
        }
        categories {
          id
          name
          slug
        }
      }
    }
  `;

  const locales = toHygraphLocales(locale);
  const data = await fetchHygraph<{ blogPosts: BlogPost[] }>(query, { locales });
  return data.blogPosts;
}

export async function getBlogPost(slug: string, locale: string = 'en'): Promise<BlogPost | null> {
  // Note: Using blogPosts (plural) with where filter because slug is not marked as unique in Hygraph schema
  const query = `
    query GetBlogPost($slug: String!, $locales: [Locale!]!) {
      blogPosts(where: { slug: $slug }, locales: $locales, stage: PUBLISHED, first: 1) {
        id
        title
        slug
        excerpt
        content {
          html
          text
        }
        publishDate
        author {
          id
          name
          bio
        }
        categories {
          id
          name
          slug
        }
      }
    }
  `;

  const locales = toHygraphLocales(locale);
  const data = await fetchHygraph<{ blogPosts: BlogPost[] }>(query, { slug, locales });
  return data.blogPosts[0] || null;
}

