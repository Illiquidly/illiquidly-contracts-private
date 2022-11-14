import { Address } from '../terra_utils';
import { env, add_contract } from '../env_helper';

/// Here we want to upload the p2p contract and add the fee contract
async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  // Uploading the contract code
  let codeId: string[] = await handler.uploadContract(
    '../artifacts/fee_distributor.wasm'
  );

  // Initialize contract
  let initMsg = {
  	name: "Fee Distributor",
  	treasury: "terra1yttw08pl3y3txd3jls4pmw5n9pesggcnta3u87ak2tddk97satasvdul7n"
  };

  let contract = await handler.instantiateContract(+codeId[0], initMsg);
  add_contract('fee_distributor', contract.address);

  console.log('Uploaded the Fee fee_distributor contract');
}

main()
  .then(() => {})
  .catch((err) => {
    console.log(err);
  });
