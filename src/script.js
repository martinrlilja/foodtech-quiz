import { h, Component, render } from 'https://unpkg.com/preact?module';
import htm from 'https://unpkg.com/htm?module';

const html = htm.bind(h);

export class Checkout {
  constructor(quizService) {
    this.quizService = quizService;
  }

  mount() {
    const forms = document.querySelectorAll('.checkout-form');

    for (const form of forms) {
      form.addEventListener('submit', async (event) => {
        event.preventDefault();
        const form_data = new FormData(form);
        const codes = form_data.get('codes')
          .split(' ')
          .map((code) => code.trim())
          .filter((code) => code.length > 0);
        const email = form_data.get('email');

        const userToken = this.quizService.getUserToken();
        const data = await this.quizService.fetchWithUser(
          '/checkout',
          userToken,
          'post',
          { codes: codes, email: email },
        );

        form.querySelector('.checkout-form__message').textContent =
          `You were registered with ${data.points} burgers! Good job!`;
      });
    }
  }
}

export class Quiz {
  constructor(quizService) {
    this.quizService = quizService;
  }

  mount(container) {
    this.updatePoints();

    const onClose = () => {
      render(html``, container);
    };

    window.addEventListener('click', async (event) => {
      const quizName = event.target.dataset.quizName;
      if (quizName) {
        const app = html`<${QuizPopup} quizName=${quizName} quizService=${this.quizService}
                                       onUpdate=${() => this.updatePoints()} onClose=${() => onClose()} />`;
        render(app, container);
      }
    });
  }

  async updatePoints() {
    let points = 0;

    const userToken = this.quizService.getUserToken();
    if (userToken) {
      const data = await this.quizService.fetchWithUser('/stats', userToken);
      points = data.total_points;
    }

    const elements = document.querySelectorAll('.hamburger-count');
    for (const element of elements) {
      element.textContent = points;
    }
  }
}

export class QuizService {
  constructor(options) {
    this.userTokenKey = 'foodtech.userToken';
    this.url = options.url;
  }

  setUserToken(token) {
    console.log(token);
    if (token) {
      window.localStorage.setItem(this.userTokenKey, token);
    }
  }

  getUserToken() {
    const token = window.localStorage.getItem(this.userTokenKey);
    if (token) {
      return `UserState ${token}`;
    } else {
      return null;
    }
  }

  async fetchWithUser(url, userToken, method, body) {
    const headers = new Headers();
    const options = {
      method: method || 'get',
    };

    if (userToken) {
      headers.append('Authorization', userToken);
    }

    if (body) {
      options.body = JSON.stringify(body);
      headers.append('Content-Type', 'application/json');
    }

    options.headers = headers;

    const response = await fetch(`${this.url}${url}`, options);
    return await response.json();
  }
}

class QuizPopup extends Component {

  constructor() {
    super();
  }

  componentDidMount() {
    this.nextQuestion();
  }

  render(props) {
    if (this.state.isAlreadyCompleted) {
      return html`<${Popup}>
        <p class="text-lg font-bold text-gray-800">You completed the quiz!</p>
        <button onClick=${() => this.props.onClose()}
                class="block w-full mt-8 px-4 py-2 rounded-lg border border-red-500 bg-transparent hover:bg-red-500 text-red-500 hover:text-white font-semibold text-lg text-center">
          Close
        </button>
      </${Popup}>`;
    } else if (this.state.choices) {
      const choices = this.state.choices.map((choice) => {
        let colors = 'border-blue-500 bg-transparent text-blue-500 hover:bg-blue-500 hover:text-white';
        if (this.state.correctAnswers) {
          if (this.state.correctAnswers.includes(choice)) {
            colors = 'border-green-500 bg-green-500 text-white';
          } else if (this.state.selectedAnswer === choice) {
            colors = 'border-red-500 bg-red-500 text-white';
          }
        } else if (this.state.selectedAnswer === choice) {
          colors = 'border-blue-500 bg-blue-500 text-white';
        }

        const isEnabled = this.state.correctAnswers === null;

        return html`<li class="px-4 my-4">
          <button disabled=${!isEnabled} onClick=${() => this.onChoice(choice)}
                  class="block w-full px-2 py-1 rounded-lg border ${colors} font-semibold text-lg text-center">
            ${choice}
          </button>
        </li>`;
      });

      const hasSelectedAnswer = this.state.selectedAnswer !== null;
      let continueColors = 'border-green-500 bg-transparent hover:bg-green-500 text-green-500 hover:text-white';
      if (!hasSelectedAnswer) {
        continueColors = 'border-gray-500 text-gray-500';
      }

      return html`<${Popup}>
        ${this.state.isCorrect === true && html`<p class="font-bold text-green-600">Correct!</p>`}
        ${this.state.isCorrect === false && html`<p class="font-bold text-red-600">Incorrect</p>`}
        ${this.state.isCorrect === null && html`<p class="text-gray-600">Question</p>`}
        <p class="text-lg font-bold text-gray-800 mb-8">${this.state.question}</p>
        <ul>
          ${choices}
        </ul>
        <button disabled=${!hasSelectedAnswer} onClick=${() => this.onContinue()}
                class="block w-full mt-8 px-4 py-2 rounded-lg border ${continueColors} font-semibold text-lg text-center">
          ${!this.state.correctAnswers && 'Continue'}
          ${!!this.state.correctAnswers && 'Next question'}
        </button>
      </${Popup}>`;
    } else {
      return html`<${Popup}><p class="text-lg font-bold text-gray-800">Loading…</p></${Popup}>`;
    }
  }

  onChoice(answer) {
    this.setState(Object.assign(this.state, { selectedAnswer: answer }));
  }

  async onContinue() {
    if (!this.state.correctAnswers) {
      const userToken = this.props.quizService.getUserToken();
      const data = await this.props.quizService.fetchWithUser(
        `/quiz/${this.props.quizName}`,
        userToken,
        'post',
        { answer: this.state.selectedAnswer },
      );

      console.log(data);
      this.setState(
        Object.assign(
          this.state,
          { isCorrect: data.is_correct, correctAnswers: data.correct },
        )
      );

      this.props.quizService.setUserToken(data.token);
      this.props.onUpdate();
    } else {
      await this.nextQuestion();
    }
  }

  async nextQuestion() {
    const userToken = this.props.quizService.getUserToken();
    const data = await this.props.quizService.fetchWithUser(`/quiz/${this.props.quizName}`, userToken);

    if (data.error === 'NotFound') {
      this.setState({ isAlreadyCompleted: true });
    } else {
      this.props.quizService.setUserToken(data.token);
      this.setState({
        question: data.question,
        choices: data.choices,
        selectedAnswer: null,
        correctAnswers: null,
        isCorrect: null,
      });
    }
  }
}

function Popup(props) {
  return html`<div class="fixed inset-0 flex justify-center items-center" style="background: rgba(0, 0, 0, 0.3)">
    <div class="bg-white shadow-2xl p-6 md:p-12 m-4 rounded-lg" style="min-width: 280px; max-width: 40rem;">
      ${props.children}
    </div>
  </div>`;
}
