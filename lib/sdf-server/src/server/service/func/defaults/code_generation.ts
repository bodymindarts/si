async function generateCode(component: Input): Promise<Output> {
  return {
    format: "json",
    code: JSON.stringify(component),
  };
}
