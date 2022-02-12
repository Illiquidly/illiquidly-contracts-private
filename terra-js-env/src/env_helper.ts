let env = require("../env.json");

if(process.argv[2]){
	env = env[process.argv[2]];
}else{
	env = env["dev"];
};
export { env };