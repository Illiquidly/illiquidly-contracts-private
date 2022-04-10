import { Address } from './terra_utils';
import { env } from './env_helper';

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  let all_handlers: Address[] = env['mnemonics'].map((mnemonic: string) => {
    return new Address(mnemonic);
  });

  for (const h of all_handlers) {
    let response = await handler.send(h.getAddress(), {
      uluna: '5000000'
    });
    console.log(response);
    console.log(
      h.getAddress(),
      (await h.terra.bank.balance(h.getAddress())).toString()
    );
  }
}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
