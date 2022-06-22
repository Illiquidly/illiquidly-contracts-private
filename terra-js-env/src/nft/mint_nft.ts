import { Address } from '../terra_utils';
import { env, add_uploaded_nft } from '../env_helper';

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  let all_handlers: Address[] = env['mnemonics'].map(
    (mnemonic: string) => new Address(mnemonic)
  );

  let cw721_tokens = env['cw721'];
  let cw721_token_names = Object.keys(cw721_tokens);
  let nft = handler.getContract(cw721_tokens[cw721_token_names[0]]);

  let token_id = "terra1kj6vwwvsw7vy7x35mazqfxyln2gk5xy00r87qy1763";
  // Mint one new nft to a specific address
  let response = await nft.execute.mint({
    token_id,
    owner: handler.getAddress(),
    token_uri: 'testing'
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
