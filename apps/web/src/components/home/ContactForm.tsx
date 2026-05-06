'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { Button } from '@/components/ui/button';

export function ContactForm() {
  const t = useTranslations('home');

  const [contactName, setContactName] = useState('');
  const [contactEmail, setContactEmail] = useState('');
  const [contactMessage, setContactMessage] = useState('');
  const [contactHoneypot, setContactHoneypot] = useState('');
  const [contactSubmitting, setContactSubmitting] = useState(false);
  const [contactSuccess, setContactSuccess] = useState(false);
  const [contactError, setContactError] = useState('');

  const handleContactSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setContactSubmitting(true);
    setContactError('');
    setContactSuccess(false);

    // Client-side validation
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    if (!emailRegex.test(contactEmail)) {
      setContactError(t('contact.errors.invalidEmail'));
      setContactSubmitting(false);
      return;
    }

    if (contactMessage.length < 10) {
      setContactError(t('contact.errors.messageTooShort'));
      setContactSubmitting(false);
      return;
    }

    if (contactMessage.length > 5000) {
      setContactError(t('contact.errors.messageTooLong'));
      setContactSubmitting(false);
      return;
    }

    try {
      const res = await fetch('/api/v1/contact', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: contactName,
          email: contactEmail,
          message: contactMessage,
          honeypot: contactHoneypot,
        }),
      });

      if (!res.ok) {
        const data = await res.json();
        throw new Error(data.error?.message || 'Failed to send message');
      }

      setContactSuccess(true);
      setContactName('');
      setContactEmail('');
      setContactMessage('');
    } catch (err) {
      setContactError(err instanceof Error ? err.message : 'Failed to send message');
    } finally {
      setContactSubmitting(false);
    }
  };

  return (
    <section className="py-20">
      <div className="max-w-2xl mx-auto px-4 sm:px-6">
        <div className="text-center mb-10">
          <span className="inline-block text-violet-600 text-sm font-medium mb-4">
            {t('contact.badge')}
          </span>
          <h2 className="text-2xl sm:text-3xl font-extrabold mb-3" style={{ color: '#333333' }}>{t('contact.title')}</h2>
          <p className="text-gray-600">{t('contact.subtitle')}</p>
        </div>

        {contactSuccess ? (
          <div className="text-center py-8">
            <div className="w-16 h-16 bg-emerald-100 rounded-full flex items-center justify-center mx-auto mb-4">
              <svg className="w-8 h-8 text-emerald-600" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M20 6L9 17l-5-5" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </div>
            <h3 className="text-xl font-bold text-gray-900 mb-2">{t('contact.successTitle')}</h3>
            <p className="text-gray-600">{t('contact.successMessage')}</p>
          </div>
        ) : (
          <form onSubmit={handleContactSubmit} className="space-y-6">
            {/* Honeypot field - hidden from users, visible to bots */}
            <div className="absolute left-[-9999px]" aria-hidden="true">
              <input
                type="text"
                name="website"
                tabIndex={-1}
                autoComplete="off"
                value={contactHoneypot}
                onChange={(e) => setContactHoneypot(e.target.value)}
              />
            </div>
            {contactError && (
              <div className="p-4 bg-red-50 border border-red-200 rounded-lg text-red-700 text-sm">
                {contactError}
              </div>
            )}
            <div>
              <label htmlFor="name" className="block text-sm font-medium text-gray-700 mb-2">{t('contact.name')}</label>
              <input
                type="text"
                id="name"
                value={contactName}
                onChange={(e) => setContactName(e.target.value)}
                required
                maxLength={100}
                className="w-full px-4 py-3 border border-gray-300 rounded-lg focus:ring-2 focus:ring-violet-500 focus:border-violet-500 outline-none transition-all"
                placeholder={t('contact.namePlaceholder')}
              />
            </div>
            <div>
              <label htmlFor="email" className="block text-sm font-medium text-gray-700 mb-2">{t('contact.email')}</label>
              <input
                type="email"
                id="email"
                value={contactEmail}
                onChange={(e) => setContactEmail(e.target.value)}
                required
                maxLength={254}
                className="w-full px-4 py-3 border border-gray-300 rounded-lg focus:ring-2 focus:ring-violet-500 focus:border-violet-500 outline-none transition-all"
                placeholder={t('contact.emailPlaceholder')}
              />
            </div>
            <div>
              <label htmlFor="message" className="block text-sm font-medium text-gray-700 mb-2">{t('contact.message')}</label>
              <textarea
                id="message"
                rows={5}
                value={contactMessage}
                onChange={(e) => setContactMessage(e.target.value)}
                required
                minLength={10}
                maxLength={5000}
                className="w-full px-4 py-3 border border-gray-300 rounded-lg focus:ring-2 focus:ring-violet-500 focus:border-violet-500 outline-none transition-all resize-none"
                placeholder={t('contact.messagePlaceholder')}
              />
            </div>
            <div className="flex gap-4">
              <Button
                type="button"
                variant="outline"
                className="flex-1 h-12 border-gray-300 hover:bg-gray-50"
                onClick={() => {
                  setContactName('');
                  setContactEmail('');
                  setContactMessage('');
                  setContactError('');
                }}
              >
                {t('contact.cancel')}
              </Button>
              <Button
                type="submit"
                disabled={contactSubmitting}
                className="flex-1 h-12 bg-violet-600 hover:bg-violet-700 text-white disabled:opacity-50"
              >
                {contactSubmitting ? t('contact.sending') : t('contact.submit')}
              </Button>
            </div>
          </form>
        )}
      </div>
    </section>
  );
}
