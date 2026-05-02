import { createI18n } from 'vue-i18n';
import en from './locales/en';
import es from './locales/es';

export const i18n = createI18n({
  legacy: false,
  locale: 'en',
  fallbackLocale: 'en',
  messages: { en, es },
});
