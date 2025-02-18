type MyType = {
  some_prop: string;
  some_num: number;
}

function useMyType<T extends MyType>(input: T): string {

  return `${input.some_prop} and ${input.some_num}`
}
