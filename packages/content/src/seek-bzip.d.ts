declare module "seek-bzip" {
  const Bunzip: {
    decode(input: Buffer, expectedSize: number): Buffer
  }
  export default Bunzip
}
