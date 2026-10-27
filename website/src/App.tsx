import { Route, Routes } from "react-router-dom";

import SiteLayout from "./components/layout/SiteLayout";
import { ROUTES } from "./config/routes";
import PortalPage from "./portal/PortalPage";
import {
  AboutPage,
  DownloadPage,
  FaqPage,
  FeaturesPage,
  HomePage,
  HowItWorksPage,
  NotFoundPage,
  PrivacyPage,
  SecurityPage,
  SupportPage,
} from "./pages";

/**
 * Route table.
 *
 * Every approved destination is a real page with its own URL. All routes share
 * one layout, so the header, footer, skip link, and `main` landmark are
 * declared once. Unknown paths render the not-found page rather than silently
 * falling back to the home page.
 */
export default function App() {
  return (
    <Routes>
      <Route element={<SiteLayout />}>
        <Route path={ROUTES.home} element={<HomePage />} />
        <Route path={ROUTES.features} element={<FeaturesPage />} />
        <Route path={ROUTES.howItWorks} element={<HowItWorksPage />} />
        <Route path={ROUTES.download} element={<DownloadPage />} />
        <Route path={ROUTES.privacy} element={<PrivacyPage />} />
        <Route path={ROUTES.security} element={<SecurityPage />} />
        <Route path={ROUTES.about} element={<AboutPage />} />
        <Route path={ROUTES.faq} element={<FaqPage />} />
        <Route path={ROUTES.support} element={<SupportPage />} />
        {/*
          Private developer portal. Both the bare path and the wildcard are
          mounted so `/admin/website` and friends render the same component,
          which resolves the section from the URL. Access is enforced by the
          portal API, not by this route table.
        */}
        <Route path={ROUTES.admin} element={<PortalPage />} />
        <Route path={`${ROUTES.admin}/*`} element={<PortalPage />} />
        <Route path="*" element={<NotFoundPage />} />
      </Route>
    </Routes>
  );
}
