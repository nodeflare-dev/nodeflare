import { getRequestConfig } from 'next-intl/server';
import { cookies, headers } from 'next/headers';
import { getLocaleFromHeader, Locale, locales } from './config';

export default getRequestConfig(async () => {
  // Check cookie first
  const cookieStore = await cookies();
  const localeCookie = cookieStore.get('locale')?.value as Locale | undefined;

  let locale: Locale;

  if (localeCookie && locales.includes(localeCookie)) {
    locale = localeCookie;
  } else {
    // Fall back to Accept-Language header
    const headerStore = await headers();
    const acceptLanguage = headerStore.get('accept-language');
    locale = getLocaleFromHeader(acceptLanguage);
  }

  return {
    locale,
    messages: (await import(`./messages/${locale}.json`)).default,
  };
});
