import { createContext, useContext, type ReactNode } from "react";
import { Icon } from "../../../components/Icon";

interface SettingsSectionValidationContextValue {
  readonly showAllErrors: boolean;
  readonly errors: readonly string[];
}

const SettingsSectionValidationContext = createContext<SettingsSectionValidationContextValue>({
  showAllErrors: false,
  errors: []
});

interface SettingsSectionValidationProviderProps {
  readonly showAllErrors: boolean;
  readonly errors?: readonly string[];
  readonly children: ReactNode;
}

export function SettingsSectionValidationProvider({
  showAllErrors,
  errors = [],
  children
}: SettingsSectionValidationProviderProps) {
  return (
    <SettingsSectionValidationContext.Provider value={{ showAllErrors, errors }}>
      {children}
    </SettingsSectionValidationContext.Provider>
  );
}

export function useSettingsSectionValidation(): boolean {
  return useContext(SettingsSectionValidationContext).showAllErrors;
}

export function SettingsSectionValidationErrors() {
  const errors = useContext(SettingsSectionValidationContext).errors;
  if (errors.length === 0) return null;

  return (
    <div className="inline-message error compact" role="alert">
      <Icon name="alert" />
      <span>{errors.join("；")}</span>
    </div>
  );
}
