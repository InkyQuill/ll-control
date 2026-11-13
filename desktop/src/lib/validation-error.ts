/** Local validation resources retain parameters until the UI renders its language. */
export class ValidationError extends Error {
  constructor(
    readonly resource: string,
    readonly values: (number | string)[] = [],
  ) {
    super(resource);
    this.name = 'ValidationError';
  }
}

export type UiDiagnostic = ValidationError | string;
