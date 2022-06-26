import { Address } from '../terra_utils';
import { env, add_uploaded_nft } from '../env_helper';

async function main() {
  // Getting a handler for the current address
  let user = new Address(env['mnemonics'][3]);
  let handler = new Address(env['mnemonics'][0]);
  let all_handlers: Address[] = env['mnemonics'].map(
    (mnemonic: string) => new Address(mnemonic)
  );

  let cw721_tokens = env['cw721'];
  let cw721_token_names = Object.keys(cw721_tokens);
  let nft = handler.getContract(cw721_tokens[cw721_token_names[0]]);

  let response = await nft.query.tokens({
    owner: user.getAddress(),
  });
  console.log(response);
  response = await nft.query.tokens({
    owner: handler.getAddress(),
  });
  console.log(response);

  let user_address = handler.getAddress();
  response = await nft.query.tokens({
    owner: user_address,
  });
  console.log(response);

}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
