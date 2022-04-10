import { Address } from './terra_utils';
import { env, add_uploaded_token } from './env_helper';

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  let all_handlers: Address[] = [];
  env['mnemonics'].forEach((mnemonic: string) => {
    all_handlers.push(new Address(mnemonic));
  });

  // Uploading the contract code
  let iliq_codeId: string[] = await handler.uploadContract(
    '../artifacts/iliq_token.wasm'
  );

  let codeName: string = 'ILLIQUIDLY TOKEN' + Math.ceil(Math.random() * 10000);

  // Instantiating the contract
  let iliqInitMsg = {
    custom: 'tesoutil',
    name: codeName,
    symbol: 'ILIQ',
    decimals: 6,
    initial_balances: all_handlers.map((handler) => {
      return {
        address: handler.getAddress(),
        amount: '1000000000'
      };
    })
  };
  let iliq = await handler.instantiateContract(+iliq_codeId[0], iliqInitMsg);
  add_uploaded_token(codeName, iliq.execute.contractAddress);
  return [codeName, iliq.execute.contractAddress];
}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
