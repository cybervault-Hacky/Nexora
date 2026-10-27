/**
 * Public API for the website's UI primitives.
 *
 * Components are imported from here so call sites stay stable if the
 * implementation files are reorganised in later phases.
 */

export { default as Button } from "./Button";
export type { ButtonProps, ButtonSize, ButtonVariant } from "./Button";

export { default as Container } from "./Container";
export type { ContainerElement, ContainerProps, ContainerWidth } from "./Container";

export { default as Eyebrow } from "./Eyebrow";
export type { EyebrowProps } from "./Eyebrow";

export { default as Section } from "./Section";
export type { SectionProps, SectionSpacing } from "./Section";

export { default as Surface } from "./Surface";
export type { SurfaceElevation, SurfacePadding, SurfaceProps } from "./Surface";
