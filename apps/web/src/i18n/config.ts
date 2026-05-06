export const locales = ['ja', 'en'] as const;
export type Locale = (typeof locales)[number];

export const defaultLocale: Locale = 'ja';

export const localeNames: Record<Locale, string> = {
  ja: '日本語',
  en: 'English',
};

export function getLocaleFromHeader(acceptLanguage: string | null): Locale {
  if (!acceptLanguage) return defaultLocale;

  const preferredLocales = acceptLanguage
    .split(',')
    .map(lang => {
      const [locale, q = '1'] = lang.trim().split(';q=');
      return { locale: locale.split('-')[0], quality: parseFloat(q) };
    })
    .sort((a, b) => b.quality - a.quality);

  for (const { locale } of preferredLocales) {
    if (locales.includes(locale as Locale)) {
      return locale as Locale;
    }
  }

  return defaultLocale;
}
